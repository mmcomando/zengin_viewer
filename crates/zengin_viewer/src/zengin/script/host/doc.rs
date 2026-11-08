use super::*;

impl mmcomando::zengine_scripts::doc_scripts::Host for ImportCallState {
    fake_impl!(doc_create() -> i32);
    fake_impl!(doc_createmap() -> i32);
    fake_impl!(doc_setpages(_0, _1));
    fake_impl!(doc_setpage(_0, _1, _2, _3));
    fake_impl!(doc_setfont(_0, _1, _2));
    fake_impl!(doc_setlevel(_0, _1));
    fake_impl!(doc_setlevelcoords(_0, _1, _2, _3, _4));
    fake_impl!(doc_setmargins(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(doc_printline(_0, _1, _2));
    fake_impl!(doc_printlines(_0, _1, _2));
    fake_impl!(doc_show(_0));
    fake_impl!(doc_open(_0));
    fake_impl!(doc_font(_0));
    fake_impl!(doc_print(_0));
    fake_impl!(doc_mapcoordinates(_0, _1, _2, _3, _4, _5, _6, _7, _8));
}
