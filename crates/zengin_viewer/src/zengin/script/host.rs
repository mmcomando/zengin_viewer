use super::bindings::mmcomando;
use super::state::ImportCallState;
use super::symbol_id::SymbolId;
use crate::warn_unimplemented;

macro_rules! fake_impl {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
            warn_unimplemented!(stringify!($name));
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            warn_unimplemented!(stringify!($name));
            0
        }
    };

}

impl mmcomando::zengine_scripts::scripts::Host for ImportCallState {
    fn info_clearchoices(&mut self, _dialog_id: i32) {
        self.dialog_state.sub_dialog_choices.clear();
    }

    fn mdl_setvisualbody(
        &mut self,
        npc_index: i32,
        body_model: i32,
        body_texture_index: i32,
        _par_4: i32,
        head_model: i32,
        face_texture_index: i32,
        _par_1: i32,
        armor_model: i32,
    ) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let entry = self.raw_instances.entry(npc_index).or_default();
        entry.body_model = self.symbol_table.string(body_model);
        entry.head_model = self.symbol_table.string(head_model);
        entry.armor_model = SymbolId::new(armor_model);
        entry.face_texture = face_texture_index;
        entry.body_texture = body_texture_index;
    }

    fn mdl_setvisual(&mut self, npc_index: i32, body_model_id: i32) {
        let Some(npc_index) = self.symbol_table.instance(npc_index) else {
            return;
        };
        let entry = self.raw_instances.entry(npc_index).or_default();
        entry.hierarchy = self.symbol_table.string(body_model_id);
    }

    fn ai_stopprocessinfos(&mut self, _npc: i32) {
        // TODO send event?
        self.dialog_state.close_dialog = true;
    }

    fn inttofloat(&mut self, v_0: i32) -> i32 {
        (v_0 as f32).to_bits() as i32
    }
    fake_impl!(inttostring(_0) -> i32);
    fake_impl!(floattostring(_0) -> i32);
    fake_impl!(floattoint(_0) -> i32);
    fake_impl!(concatstrings(_0, _1) -> i32);
    fake_impl!(print(_0));
    fake_impl!(printdebug(_0));
    fake_impl!(printscreen(_0, _1, _2, _3, _4) -> i32);
    fake_impl!(ai_printscreen(_0, _1, _2, _3, _4) -> i32);
    fake_impl!(printdialog(_0, _1, _2, _3, _4, _5) -> i32);
    fake_impl!(printdebuginst(_0));
    fake_impl!(printdebuginstch(_0, _1));
    fake_impl!(printdebugch(_0, _1));
    fake_impl!(printmulti(_0, _1, _2, _3, _4));
    fake_impl!(exitgame());
    fake_impl!(exitsession());
    fake_impl!(playvideo(_0) -> i32);
    fake_impl!(playvideoex(_0, _1, _2) -> i32);
    fake_impl!(setpercentdone(_0));
    fake_impl!(introducechapter(_0, _1, _2, _3, _4));
    fake_impl!(npc_memoryentry(_0, _1, _2, _3, _4));
    fake_impl!(npc_memoryentryguild(_0, _1, _2, _3, _4));
    fake_impl!(npc_hasnews(_0, _1, _2, _3) -> i32);
    fake_impl!(npc_getnewswitness(_0, _1) -> i32);
    fake_impl!(npc_getnewsoffender(_0, _1) -> i32);
    fake_impl!(npc_getnewsvictim(_0, _1) -> i32);
    fake_impl!(npc_isnewsgossip(_0, _1) -> i32);
    fake_impl!(npc_deletenews(_0, _1) -> i32);
    fake_impl!(npc_getguildattitude(_0, _1) -> i32);
    fake_impl!(npc_getactivespell(_0) -> i32);
    fake_impl!(npc_getlasthitspellid(_0) -> i32);
    fake_impl!(npc_getlasthitspellcat(_0) -> i32);
    fake_impl!(npc_getactivespellcat(_0) -> i32);
    fake_impl!(npc_getactivespelllevel(_0) -> i32);
    fake_impl!(npc_getactivespellisscroll(_0) -> i32);
    fake_impl!(npc_setactivespellinfo(_0, _1) -> i32);
    fake_impl!(wld_detectitem(_0, _1) -> i32);
    fake_impl!(wld_detectplayer(_0) -> i32);
    fake_impl!(wld_detectnpc(_0, _1, _2, _3) -> i32);
    fake_impl!(wld_detectnpcex(_0, _1, _2, _3, _4) -> i32);
    fake_impl!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    fake_impl!(wld_setguildattitude(_0, _1, _2));
    fake_impl!(wld_getguildattitude(_0, _1) -> i32);
    /// Minimal mob availability: all mobs count as available so routine states
    /// reach their use-mob branch (real mob objects are not simulated)
    fn wld_ismobavailable(&mut self, _npc: i32, _mob_name: i32) -> i32 {
        warn_unimplemented!("wld_ismobavailable");
        1
    }
    fake_impl!(wld_isfpavailable(_0, _1) -> i32);
    fake_impl!(wld_isnextfpavailable(_0, _1) -> i32);
    fake_impl!(npc_createspell(_0, _1));
    fake_impl!(npc_learnspell(_0, _1));

    fn npc_hasitems(&mut self, _npc: i32, _item: i32) -> i32 {
        warn_unimplemented!("npc_hasitems");
        1
    }
    fake_impl!(npc_giveitem(_0, _1, _2));
    fake_impl!(createinvitem(_0, _1));
    fake_impl!(createinvitems(_0, _1, _2));
    fake_impl!(npc_getinvitem(_0, _1) -> i32);
    fake_impl!(npc_getinvitembyslot(_0, _1, _2) -> i32);
    fake_impl!(npc_removeinvitem(_0, _1) -> i32);
    fake_impl!(npc_removeinvitems(_0, _1, _2) -> i32);
    fake_impl!(npc_clearinventory(_0));
    fake_impl!(npc_isinstate(_0, _1) -> i32);
    fake_impl!(npc_wasinstate(_0, _1) -> i32);
    fake_impl!(npc_isinroutine(_0, _1) -> i32);
    fake_impl!(ai_setnpcstostate(_0, _1, _2));
    fake_impl!(npc_hasdetectednpc(_0, _1) -> i32);
    fake_impl!(npc_setattitude(_0, _1));
    fake_impl!(npc_settempattitude(_0, _1));
    fake_impl!(npc_getattitude(_0, _1) -> i32);
    fake_impl!(npc_getpermattitude(_0, _1) -> i32);
    fake_impl!(npc_changeattribute(_0, _1, _2));
    fake_impl!(npc_getcomrades(_0) -> i32);
    fake_impl!(npc_isnear(_0, _1) -> i32);
    fake_impl!(npc_getdisttonpc(_0, _1) -> i32);
    fake_impl!(npc_getheighttonpc(_0, _1) -> i32);
    fake_impl!(npc_getheighttoitem(_0, _1) -> i32);
    fake_impl!(npc_getdisttoplayer(_0) -> i32);
    fake_impl!(npc_getdisttoitem(_0, _1) -> i32);
    fake_impl!(npc_getdisttowp(_0, _1) -> i32);
    fake_impl!(npc_canseenpc(_0, _1) -> i32);
    fake_impl!(npc_canseenpcfreelos(_0, _1) -> i32);
    fake_impl!(npc_canseeitem(_0, _1) -> i32);
    fake_impl!(npc_isplayer(_0) -> i32);
    fake_impl!(npc_knowsplayer(_0, _1) -> i32);
    fake_impl!(npc_setknowsplayer(_0, _1));
    fake_impl!(npc_isinfightmode(_0, _1) -> i32);
    fake_impl!(npc_isaiming(_0, _1) -> i32);
    fake_impl!(npc_startitemreactmodules(_0, _1, _2) -> i32);
    fake_impl!(npc_hasoffered(_0, _1, _2) -> i32);
    fake_impl!(equipitem(_0, _1));
    fake_impl!(npc_settofistmode(_0));
    fake_impl!(npc_settofightmode(_0, _1));
    fake_impl!(npc_isdead(_0) -> i32);
    fake_impl!(ai_startstate(_0, _1, _2, _3));
    fake_impl!(ai_continueroutine(_0));
    fn npc_getstatetime(&mut self, npc: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        self.state_times.get(&npc).copied().unwrap_or(0)
    }
    fn npc_setstatetime(&mut self, npc: i32, seconds: i32) {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return;
        };
        self.state_times.insert(npc, seconds);
    }
    fake_impl!(ai_outputsvm(_0, _1, _2));
    fake_impl!(ai_outputsvm_overlay(_0, _1, _2));
    fake_impl!(ai_playcutscene(_0, _1));
    fake_impl!(ai_canseenpc(_0, _1, _2));
    fake_impl!(ai_wait(_0, _1));
    fake_impl!(ai_waitms(_0, _1));
    fake_impl!(ai_waittillend(_0, _1));
    fake_impl!(ai_aligntowp(_0));
    fake_impl!(ai_setwalkmode(_0, _1));
    fake_impl!(ai_gotowp(_0, _1));
    fake_impl!(ai_teleport(_0, _1));
    fake_impl!(ai_gotoitem(_0, _1));
    fake_impl!(ai_gotonpc(_0, _1));
    fake_impl!(ai_aligntofp(_0));
    fake_impl!(ai_gotofp(_0, _1));
    fake_impl!(npc_isonfp(_0, _1) -> i32);
    fake_impl!(ai_gotonextfp(_0, _1));
    fake_impl!(ai_gotosound(_0));
    fake_impl!(ai_takeitem(_0, _1));
    fake_impl!(ai_dropitem(_0, _1));
    fake_impl!(ai_drawweapon(_0));
    fake_impl!(ai_removeweapon(_0));
    fake_impl!(npc_hasrangedweaponwithammo(_0) -> i32);
    fake_impl!(npc_hasequippedweapon(_0) -> i32);
    fake_impl!(npc_hasequippedmeleeweapon(_0) -> i32);
    fake_impl!(npc_hasequippedrangedweapon(_0) -> i32);
    fake_impl!(npc_hasequippedarmor(_0) -> i32);
    fake_impl!(npc_hasreadiedweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedmeleeweapon(_0) -> i32);
    fake_impl!(npc_hasreadiedrangedweapon(_0) -> i32);
    fake_impl!(npc_gettrueguild(_0) -> i32);
    fake_impl!(npc_settrueguild(_0, _1) -> i32);
    fake_impl!(npc_arewestronger(_0, _1) -> i32);
    fake_impl!(npc_gettarget(_0) -> i32);
    fake_impl!(npc_getnexttarget(_0) -> i32);
    fake_impl!(npc_getlookattarget(_0) -> i32);
    fake_impl!(npc_isnexttargetavailable(_0) -> i32);
    fake_impl!(npc_settarget(_0, _1));
    fake_impl!(npc_getbodystate(_0) -> i32);
    fake_impl!(npc_hasbodyflag(_0, _1) -> i32);
    fake_impl!(npc_isincutscene(_0) -> i32);
    fake_impl!(npc_isvoiceactive(_0) -> i32);
    fake_impl!(npc_stopani(_0, _1));
    fake_impl!(npc_getdetectedmob(_0) -> i32);
    fake_impl!(npc_playani(_0, _1));
    fake_impl!(npc_isdetectedmobownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_isdetectedmobownedbyguild(_0, _1) -> i32);
    fake_impl!(wld_getmobstate(_0, _1) -> i32);
    fake_impl!(npc_clearaiqueue(_0));
    fake_impl!(infomanager_hasfinished() -> i32);
    fake_impl!(ai_processinfos(_0));

    /// Knowledge is gained when a dialog root is played.
    fn npc_knowsinfo(&mut self, npc: i32, info: i32) -> i32 {
        let Some(npc) = self.symbol_table.instance(npc) else {
            return 0;
        };
        let Some(info) = self.symbol_table.instance(info) else {
            return 0;
        };
        self.known_infos
            .get(&npc)
            .is_some_and(|infos| infos.contains(&info)) as i32
    }
    fake_impl!(npc_checkinfo(_0, _1) -> i32);
    fake_impl!(npc_giveinfo(_0, _1) -> i32);
    fake_impl!(npc_gettalentskill(_0, _1) -> i32);
    fake_impl!(npc_gettalentvalue(_0, _1) -> i32);
    fake_impl!(npc_settalentskill(_0, _1, _2));
    fake_impl!(npc_settalentvalue(_0, _1, _2));
    fake_impl!(tal_configure(_0, _1));
    fake_impl!(npc_getnearestwp(_0) -> i32);
    fake_impl!(npc_getnextwp(_0) -> i32);
    fake_impl!(npc_iswayblocked(_0) -> i32);
    fake_impl!(npc_getreadiedweapon(_0) -> i32);
    fake_impl!(npc_getequippedmeleeweapon(_0) -> i32);
    fake_impl!(npc_getequippedrangedweapon(_0) -> i32);
    fake_impl!(npc_getequippedarmor(_0) -> i32);
    fake_impl!(ai_equipbestmeleeweapon(_0) -> i32);
    fake_impl!(ai_equipbestrangedweapon(_0) -> i32);
    fake_impl!(ai_equipbestarmor(_0));
    fake_impl!(ai_unequipweapons(_0));
    fake_impl!(ai_unequiparmor(_0));
    fake_impl!(ai_equiparmor(_0, _1));
    fake_impl!(ai_readymeleeweapon(_0));
    fake_impl!(ai_readyrangedweapon(_0));
    fake_impl!(ai_turnaway(_0, _1));
    fake_impl!(ai_turntonpc(_0, _1));
    fake_impl!(ai_whirlaround(_0, _1));
    fake_impl!(ai_whirlaroundtosource(_0));
    fake_impl!(ai_turntosound(_0));
    fake_impl!(ai_quicklook(_0, _1));
    fake_impl!(ai_lookat(_0, _1));
    fake_impl!(ai_lookatnpc(_0, _1));
    fake_impl!(ai_stoplookat(_0));
    fake_impl!(ai_pointat(_0, _1));
    fake_impl!(ai_pointatnpc(_0, _1));
    fake_impl!(ai_stoppointat(_0));
    fake_impl!(ai_standup(_0));
    fake_impl!(ai_standupquick(_0));
    fake_impl!(ai_flee(_0));
    fake_impl!(ai_aimat(_0, _1));
    fake_impl!(ai_stopaim(_0));
    fake_impl!(ai_shootat(_0, _1));
    fake_impl!(ai_defend(_0));
    fake_impl!(ai_combatreacttodamage(_0));
    fake_impl!(ai_readyspell(_0, _1, _2));
    fake_impl!(ai_unreadyspell(_0));
    fake_impl!(npc_hasspell(_0, _1) -> i32);
    fake_impl!(ai_attack(_0));
    fake_impl!(ai_finishingmove(_0, _1));
    fake_impl!(ai_dodge(_0));
    fake_impl!(ai_useitem(_0, _1));
    fake_impl!(ai_useitemtostate(_0, _1, _2));
    fake_impl!(npc_refusetalk(_0) -> i32);
    fake_impl!(npc_setrefusetalk(_0, _1));
    fake_impl!(ai_ask(_0, _1, _2));
    fake_impl!(ai_asktext(_0, _1, _2, _3, _4));
    fake_impl!(ai_waitforquestion(_0, _1));
    fake_impl!(hlp_isitem(_0, _1) -> i32);
    fake_impl!(hlp_getinstanceid(_0) -> i32);
    fake_impl!(hlp_random(_0) -> i32);
    fake_impl!(hlp_strcmp(_0, _1) -> i32);
    fake_impl!(hlp_getnpc(_0) -> i32);
    /// The viewer has no separate npc handle table, an npc argument is valid when
    /// it resolves to an instance symbol (the resolution every other `npc_*` host
    /// call performs).
    fn hlp_isvalidnpc(&mut self, npc: i32) -> i32 {
        self.symbol_table.instance(npc).is_some() as i32
    }
    fake_impl!(hlp_isvaliditem(_0) -> i32);
    fake_impl!(ai_snd_play(_0, _1));
    fake_impl!(ai_snd_play3d(_0, _1, _2));
    fake_impl!(snd_play(_0));
    fake_impl!(snd_play3d(_0, _1));
    fake_impl!(snd_getdisttosource(_0) -> i32);
    fake_impl!(snd_issourcenpc(_0) -> i32);
    fake_impl!(snd_issourceitem(_0) -> i32);
    fake_impl!(npc_canseesource(_0) -> i32);
    fake_impl!(mis_getstatus(_0) -> i32);
    fake_impl!(mis_setstatus(_0, _1));
    fake_impl!(mis_ontime(_0) -> i32);
    fake_impl!(log_createtopic(_0, _1));
    fake_impl!(log_addentry(_0, _1));
    fake_impl!(log_settopicstatus(_0, _1));
    fake_impl!(mis_addmissionentry(_0, _1));
    fake_impl!(mis_removemission(_0));
    fake_impl!(npc_checkavailablemission(_0, _1, _2) -> i32);
    fake_impl!(npc_checkrunningmission(_0, _1) -> i32);
    fake_impl!(npc_isdrawingweapon(_0) -> i32);
    fake_impl!(npc_isdrawingspell(_0) -> i32);
    fake_impl!(npc_checkoffermission(_0, _1) -> i32);
    fake_impl!(mdl_applyoverlaymds(_0, _1));
    fake_impl!(mdl_applyoverlaymdstimed(_0, _1, _2));
    fake_impl!(mdl_removeoverlaymds(_0, _1));
    fake_impl!(mdl_applyrandomani(_0, _1, _2));
    fake_impl!(mdl_applyrandomanifreq(_0, _1, _2));
    fake_impl!(mdl_setmodelscale(_0, _1, _2, _3));
    fake_impl!(mdl_setmodelfatness(_0, _1));
    fake_impl!(mdl_startfaceani(_0, _1, _2, _3));
    fake_impl!(mdl_applyrandomfaceani(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_istime(_0, _1, _2, _3) -> i32);
    fake_impl!(wld_getday() -> i32);
    fake_impl!(wld_israining() -> i32);
    fake_impl!(wld_settime(_0, _1));
    fake_impl!(wld_spawnnpcrange(_0, _1, _2, _3));
    fake_impl!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    fake_impl!(wld_stopeffect(_0));
    fake_impl!(ai_playfx(_0, _1, _2));
    fake_impl!(ai_stopfx(_0, _1));
    fake_impl!(wld_removenpc(_0));
    fake_impl!(wld_insertnpcandrespawn(_0, _1, _2));
    fake_impl!(wld_insertobject(_0, _1));
    fake_impl!(wld_removeitem(_0) -> i32);
    fake_impl!(wld_exchangeguildattitudes(_0));
    fake_impl!(wld_setobjectroutine(_0, _1, _2, _3));
    fake_impl!(wld_setmobroutine(_0, _1, _2, _3));
    fake_impl!(wld_sendtrigger(_0));
    fake_impl!(wld_senduntrigger(_0));
    fake_impl!(npc_ownedbynpc(_0, _1) -> i32);
    fake_impl!(npc_ownedbyguild(_0, _1) -> i32);
    fake_impl!(ai_takemob(_0, _1));
    fake_impl!(ai_dropmob(_0));
    fake_impl!(mob_createitems(_0, _1, _2));
    fake_impl!(mob_hasitems(_0, _1) -> i32);
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
    fake_impl!(ta(_0, _1, _2, _3, _4));
    fake_impl!(ta_cs(_0, _1, _2));
    fake_impl!(ta_beginoverlay(_0));
    fake_impl!(ta_endoverlay(_0));
    fake_impl!(ta_removeoverlay(_0));
    fake_impl!(npc_exchangeroutine(_0, _1));
    fake_impl!(rtn_exchange(_0, _1));
    fake_impl!(perc_setrange(_0, _1));
    fake_impl!(npc_setperctime(_0, _1));
    fake_impl!(npc_percenable(_0, _1, _2));
    fake_impl!(npc_percdisable(_0, _1));
    fake_impl!(npc_sendpassiveperc(_0, _1, _2, _3));
    fake_impl!(npc_sendsingleperc(_0, _1, _2));
    fake_impl!(npc_perceiveall(_0));
    fake_impl!(wld_assignroomtoguild(_0, _1));
    fake_impl!(wld_assignroomtonpc(_0, _1));
    fake_impl!(wld_getplayerportalowner() -> i32);
    fake_impl!(wld_getplayerportalguild() -> i32);
    fake_impl!(wld_getformerplayerportalowner() -> i32);
    fake_impl!(wld_getformerplayerportalguild() -> i32);
    fake_impl!(npc_isplayerinmyroom(_0) -> i32);
    fake_impl!(npc_isinplayersroom(_0) -> i32);
    fake_impl!(npc_wasplayerinmyroom(_0) -> i32);
    fake_impl!(npc_getportalowner(_0) -> i32);
    fake_impl!(npc_getportalguild(_0) -> i32);
    fake_impl!(hlp_cutsceneplayed(_0) -> i32);
    fake_impl!(game_initgerman() -> i32);
    fake_impl!(game_initenglish() -> i32);
    fake_impl!(game_initengintl() -> i32);
}
