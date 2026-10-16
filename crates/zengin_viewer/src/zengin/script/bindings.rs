use wasmtime::component::bindgen;

bindgen!({
    // Same WIT package daedalus_wasm embeds into the produced component
    path:"../daedalus_wasm/wit",
});
