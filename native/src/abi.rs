//! C layout of Online Deluxe plugin API v1, descriptor revisions 4 and 5.
//!
//! Copied from the Deluxe host at
//! `/home/intpa/ssbu-online-deluxe-rewrite` commit `78d90e1`
//! (`sdk/abi/src/lib.rs`). The host copies every string during `register`.
//! `user`, `current` and `request` stay for the process lifetime. Revision 5 has
//! the same layout; it only adds the cell option flag (`OPTION_FLAG_CELL`), and
//! a host without it answers `REGISTER_ERR_ABI` before registering anything.

use core::ffi::{c_char, c_void};

pub const TABLE_VERSION: u32 = 1;
pub const REGISTER_ERR_ABI: i32 = -2;
pub const REGISTER_ERR_INVALID: i32 = -3;
pub const API_SYMBOL: &[u8] = b"ssbu_online_deluxe_plugin_api_v1\0";

pub type CurrentFn = unsafe extern "C" fn(user: *mut c_void, option: u32) -> u32;
pub type RequestFn = unsafe extern "C" fn(user: *mut c_void, option: u32, choice: u32) -> u32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OptionDesc {
    pub id: *const c_char,
    pub label: *const c_char,
    pub choices: *const *const c_char,
    pub choice_count: u32,
    pub flags: u32,
    pub config_key: *const c_char,
    pub choice_values: *const *const c_char,
    pub choice_tones: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginDesc {
    pub struct_size: u32,
    pub abi_version: u32,
    pub id: *const c_char,
    pub label: *const c_char,
    pub options: *const OptionDesc,
    pub option_count: u32,
    pub flags: u32,
    pub user: *mut c_void,
    pub current: Option<CurrentFn>,
    pub request: Option<RequestFn>,
    pub config_path: *const c_char,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginApiV1 {
    pub struct_size: u32,
    pub abi_version: u32,
    pub register: unsafe extern "C" fn(desc: *const PluginDesc) -> i32,
    pub host_status: unsafe extern "C" fn() -> u32,
    pub host_build: unsafe extern "C" fn() -> *const c_char,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    #[test]
    fn layout_matches_deluxe_plugin_api_v1() {
        assert_eq!(size_of::<OptionDesc>(), 56);
        assert_eq!(size_of::<PluginDesc>(), 72);
        assert_eq!(size_of::<PluginApiV1>(), 32);
        assert_eq!(offset_of!(OptionDesc, id), 0);
        assert_eq!(offset_of!(OptionDesc, label), 8);
        assert_eq!(offset_of!(OptionDesc, choices), 16);
        assert_eq!(offset_of!(OptionDesc, choice_count), 24);
        assert_eq!(offset_of!(OptionDesc, flags), 28);
        assert_eq!(offset_of!(OptionDesc, config_key), 32);
        assert_eq!(offset_of!(OptionDesc, choice_values), 40);
        assert_eq!(offset_of!(OptionDesc, choice_tones), 48);
        assert_eq!(offset_of!(PluginDesc, struct_size), 0);
        assert_eq!(offset_of!(PluginDesc, abi_version), 4);
        assert_eq!(offset_of!(PluginDesc, id), 8);
        assert_eq!(offset_of!(PluginDesc, label), 16);
        assert_eq!(offset_of!(PluginDesc, options), 24);
        assert_eq!(offset_of!(PluginDesc, option_count), 32);
        assert_eq!(offset_of!(PluginDesc, flags), 36);
        assert_eq!(offset_of!(PluginDesc, user), 40);
        assert_eq!(offset_of!(PluginDesc, current), 48);
        assert_eq!(offset_of!(PluginDesc, request), 56);
        assert_eq!(offset_of!(PluginDesc, config_path), 64);
        assert_eq!(offset_of!(PluginApiV1, struct_size), 0);
        assert_eq!(offset_of!(PluginApiV1, abi_version), 4);
        assert_eq!(offset_of!(PluginApiV1, register), 8);
        assert_eq!(offset_of!(PluginApiV1, host_status), 16);
        assert_eq!(offset_of!(PluginApiV1, host_build), 24);
        assert_eq!(TABLE_VERSION, 1);
        assert_eq!(REGISTER_ERR_ABI, -2);
        assert_eq!(REGISTER_ERR_INVALID, -3);
        assert!(API_SYMBOL.ends_with(&[0]));
    }
}
