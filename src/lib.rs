// dummy root package for worker-build virtual manifest fix
// Ensure wasm has a function table for --experimental-reset-state-function
#[no_mangle]
pub extern "C" fn _dummy_meting_api_rs_table() {}
