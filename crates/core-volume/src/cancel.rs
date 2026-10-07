/// Whether the caller has given up on a run, asked between the pieces of work it is made
/// of.
///
/// A field over a real model takes seconds, and a Ctrl-C has to land inside them rather
/// than after, the way placement already stops inside a block of layers (ADR 0033). A run
/// nothing stops passes [`Cancel::never`].
#[derive(Clone, Copy, Default)]
pub struct Cancel<'a>(Option<&'a (dyn Fn() -> bool + Sync)>);

impl<'a> Cancel<'a> {
    /// A run that is never given up on.
    pub const fn never() -> Self {
        Self(None)
    }

    /// A run that stops once `asked` answers true.
    pub const fn when(asked: &'a (dyn Fn() -> bool + Sync)) -> Self {
        Self(Some(asked))
    }

    /// Whether the run has been given up on. Asked often, so it does no work of its own.
    pub fn asked(&self) -> bool {
        self.0.is_some_and(|asked| asked())
    }
}
