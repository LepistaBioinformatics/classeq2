pub(crate) enum PlacementResult<T, U> {
    Success(T),
    Error(U),
}
