use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};

use futures::{
    Stream,
    StreamExt,
};
use k8s_openapi::api::core::v1 as corev1;
use kube::runtime::watcher::{
    watcher,
    Error,
    Event,
};
use tracing::*;

use crate::util::{
    Clockable,
    UtcClock,
};
use crate::watchertracer::tracer::Tracer;

pub type PodStream = Pin<Box<dyn Stream<Item = Result<Event<corev1::Pod>, Error>> + Send>>;

pub struct Watcher {
    w: PodStream,
    t: Arc<Mutex<Tracer>>,
    clock: Arc<Mutex<dyn Clockable>>,
}

impl Watcher {
    pub fn new(client: kube::Client, t: Arc<Mutex<Tracer>>) -> Watcher {
        let pods: kube::Api<corev1::Pod> = kube::Api::all(client);
        return Watcher {
            w: watcher(pods, Default::default()).boxed(),
            clock: Arc::new(Mutex::new(UtcClock {})),
            t,
        };
    }

    pub fn new_from_parts(w: PodStream, t: Arc<Mutex<Tracer>>, clock: Arc<Mutex<dyn Clockable>>) -> Watcher {
        return Watcher { w, t, clock };
    }

    pub async fn start(&mut self) {
        loop {
            tokio::select! {
                item = self.w.next() => match item {
                    None => break,
                    Some(res) => match res {
                        Ok(evt) => self.handle_pod_event(evt),
                        Err(e) => error!("tracer received error on stream: {}", e),
                    },
                },
            }
        }
    }

    fn handle_pod_event(&mut self, evt: Event<corev1::Pod>) {
        let ts = self.clock.lock().unwrap().now();
        let mut tracer = self.t.lock().unwrap();

        match evt {
            Event::Applied(pod) => tracer.create_pod(&pod, ts),
            Event::Deleted(pod) => tracer.delete_pod(&pod, ts),
            Event::Restarted(pods) => tracer.update_all_pods(pods, ts),
        }
    }
}
