use pin_project::pin_project;

/// A pin-project compatible `Option`
#[pin_project(project = OptionPinProj)]
pub(crate) enum OptionPin<T> {
    Some(#[pin] T),
    None,
}
