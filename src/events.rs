use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::Uri;
use futures::Future;
use prism_client::{AsyncClient, Wavelet};
use subseq_util::tables::DbPool;
use tokio::spawn;
use tokio::sync::broadcast;
use subseq_util::router::Router;

use crate::tables::User;


pub fn prism_url(host: &str, port: u16) -> String {
    format!("ws://{}:{}", host, port)
}

#[derive(Clone)]
pub struct UserCreated(pub User);

const USER_CREATED_BEAM: &str = "urn:subseq.io:oidc:user:created";
const USER_UPDATED_BEAM: &str = "urn:subseq.io:oidc:user:updated";

pub fn create_users_from_events(mut user_created_rx: broadcast::Receiver<UserCreated>,
                                db_pool: Arc<DbPool>) {
    spawn(async move {
        while let Ok(created_user) = user_created_rx.recv().await {
            let mut conn = match db_pool.get() {
                Ok(conn) => conn,
                Err(_) => return
            };
            let user = created_user.0;
            if User::get(&mut conn, user.id).is_none() {
                User::create(&mut conn, user.id, &user.email, None).ok();
            }
        }
    });
}


struct WaveletHandler {
    user_created_tx: broadcast::Sender<UserCreated>,
    wavelet: Wavelet,
}

impl Future for WaveletHandler {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let Wavelet { beam, photons } = &this.wavelet;
        match beam.as_str() {
            b => {
                tracing::error!("Received unhandled Beam: {}", b);
            }
        }
        Poll::Ready(())
    }
}


pub fn emit_events(addr: &str, mut router: Router, db_pool: Arc<DbPool>) {
    let mut user_rx: broadcast::Receiver<User> = router.subscribe();

    let user_created_tx: broadcast::Sender<UserCreated> = router.announce();
    let user_created_rx: broadcast::Receiver<UserCreated> = router.subscribe();
    create_users_from_events(user_created_rx, db_pool);
    let uri = addr.parse::<Uri>().unwrap();

    spawn(async move {
        let handle_tasks = move |wavelet: Wavelet| {
            WaveletHandler {
                user_created_tx: user_created_tx.clone(),
                wavelet
            }
        };

        let mut client = match AsyncClient::connect(uri, handle_tasks).await {
            Ok(client) => client,
            Err(_err) => {
                tracing::warn!("Zini is running in standalone mode. No connection to prism.");
                return;
            }
        };
        tracing::info!("Zini connected to prism!");
        client.add_beam(USER_CREATED_BEAM).await.expect("Failed setting up client");

        loop {
            tokio::select!(
                msg = user_rx.recv() => {
                    if let Ok(msg) = msg {
                        let vec = serde_json::to_vec(&msg).unwrap();
                        if client.emit(USER_CREATED_BEAM, vec).await.is_err() {
                            break;
                        }
                    }
                }
            )
        }
        tracing::warn!("Zini client closed");
    });
}
