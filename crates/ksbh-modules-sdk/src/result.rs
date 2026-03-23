/// Result type returned by a module's `process` function.
///
/// Determines how request processing continues:
/// - `Pass`: Continue to the next module or backend
/// - `Stop(Response)`: Halt processing and return the response immediately to the client
pub enum ModuleResult {
    /// Continue request processing to the next module or to the backend.
    Pass,
    /// Stop processing and return this response to the client immediately.
    /// Other modules will NOT be called.
    Stop(http::Response<bytes::Bytes>),
}
