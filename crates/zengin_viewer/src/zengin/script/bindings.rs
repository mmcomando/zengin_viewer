use wasmtime::component::bindgen;

bindgen!({
    // Same WIT package daedalus_wasm embeds into the produced component
    path:"../daedalus_wasm/wit",

    imports: {
        "mmcomando:zengine-scripts/scripts.wld-insertnpc": store,
        "mmcomando:zengine-scripts/scripts.ta-min": store,
        "mmcomando:zengine-scripts/scripts.ai-playani": store,
        "mmcomando:zengine-scripts/scripts.ai-playanibs": store,
        "mmcomando:zengine-scripts/scripts.ai-usemob": store,
        "mmcomando:zengine-scripts/scripts.wld-insertitem": store,
        "mmcomando:zengine-scripts/scripts.ai-output": store,
        "mmcomando:zengine-scripts/scripts.info-addchoice": store,
        "mmcomando:zengine-scripts/scripts.hlp-strcmp": store,
    },

});
