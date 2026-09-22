pub mod lcu;
pub mod regions;
pub mod sgp;
#[cfg(windows)]
mod windows;

pub fn client_error(error: lcu::LcuError) -> String {
    format!("client.{error:?}")
}
pub async fn connect() -> Result<lcu::LocalClient, String> {
    tokio::task::spawn_blocking(lcu::discover)
        .await
        .map_err(|_| "client.Discovery")?
        .map_err(client_error)
}
