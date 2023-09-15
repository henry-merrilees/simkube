use std::sync::{
    Arc,
    Mutex,
};

use futures::future::try_join_all;
use futures::stream::select_all::{
    select_all,
    SelectAll,
};
use futures::stream::TryStreamExt;
use futures::StreamExt;
use kube::api::{
    DynamicObject,
    GroupVersionKind,
};
use kube::core::TypeMeta;
use kube::runtime::watcher::{
    watcher,
    Event,
};
use kube::runtime::WatchStreamExt;
use tracing::*;

use super::KubeObjectStream;
use crate::prelude::*;
use crate::trace::Tracer;
use crate::util::{
    get_api_resource,
    strip_obj,
    Clockable,
    UtcClock,
};

pub struct Watcher<'a> {
    w: SelectAll<KubeObjectStream<'a>>,
    t: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}

async fn build_stream_for<'a>(
    gvk: &'a GroupVersionKind,
    obj_cfg: &'a TrackedObject,
    client: kube::Client,
) -> anyhow::Result<KubeObjectStream<'a>> {
    let (ar, _) = get_api_resource(gvk, &client).await?;
    Ok(watcher(kube::Api::all_with(client, &ar), Default::default())
        .modify(|obj| {
            strip_obj(obj, &obj_cfg.pod_spec_path);
            obj.types = Some(TypeMeta {
                api_version: gvk.api_version(),
                kind: gvk.kind.clone(),
            });
        })
        .map_err(|e| e.into())
        .boxed())
}

impl<'a> Watcher<'a> {
    pub async fn new(client: kube::Client, t: Arc<Mutex<Tracer>>, config: &'a TracerConfig) -> anyhow::Result<Watcher> {
        let apis = try_join_all(
            config
                .tracked_objects
                .iter()
                .map(|(key, obj_cfg)| build_stream_for(&key.gvk, obj_cfg, client.clone())),
        )
        .await?;

        Ok(Watcher {
            w: select_all(apis),
            clock: Arc::new(Mutex::new(UtcClock {})),
            t,
        })
    }

    pub fn new_from_parts(w: KubeObjectStream, t: Arc<Mutex<Tracer>>, clock: Arc<Mutex<dyn Clockable>>) -> Watcher {
        Watcher { w: select_all(vec![w]), t, clock }
    }

    pub async fn start(&mut self) {
        loop {
            tokio::select! {
                item = self.w.next() => if let Some(res) = item {
                    match res {
                        Ok(evt) => self.handle_pod_event(evt),
                        Err(e) => error!("tracer received error on stream: {}", e),
                    }
                } else { break },
            }
        }
    }

    fn handle_pod_event(&mut self, evt: Event<DynamicObject>) {
        let ts = self.clock.lock().unwrap().now();
        let mut tracer = self.t.lock().unwrap();

        match evt {
            Event::Applied(obj) => tracer.create_obj(&obj, ts),
            Event::Deleted(obj) => tracer.delete_obj(&obj, ts),
            Event::Restarted(objs) => tracer.update_all_objs(objs, ts),
        }
    }
}
