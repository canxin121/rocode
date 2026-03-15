#[path = "app.rs"]
mod app_impl;
mod state;
mod terminal;

pub use app_impl::App;
pub use state::AppState;
