use super::Outcome;

pub trait Callable<M>
where
    Self: Send,
    // why is this here?
    M: Send,
{
    type Outcome: Outcome + Send;
    fn call_func(&mut self, msg: &M) -> Self::Outcome;
}

impl<F, M, O> Callable<M> for F
where
    F: FnMut(&M) -> O + Send,
    M: Send,
    O: Outcome + Send,
{
    type Outcome = O;

    fn call_func(&mut self, msg: &M) -> Self::Outcome {
        (self)(msg)
    }
}
