/// app routes
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Routes {
    Home,
    Settings,
}

/// Navigate event
#[derive(Clone, Copy)]
pub struct Navigate(pub Routes);
