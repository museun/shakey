use super::Outcome;

pub trait Callable<M> {
    type Outcome: Outcome;
    fn call_func(&mut self, msg: &M) -> Self::Outcome;
}

impl<F, M, O> Callable<M> for F
where
    F: FnMut(&M) -> O,
    O: Outcome,
{
    type Outcome = O;

    fn call_func(&mut self, msg: &M) -> Self::Outcome {
        (self)(msg)
    }
}
