// SPDX-License-Identifier: AGPL-3.0-or-later WITH GPL-3.0-linking-exception
// SPDX-FileCopyrightText: 2021 Alyssa Ross <hi@alyssa.is>

use std::io;
use std::os::raw::{c_char, c_int, c_uint};
use std::os::unix::prelude::*;
use std::ptr::null;

extern "C" {
    fn sd_listen_fds(unset_environment: c_int) -> c_int;
    fn sd_is_socket_inet(
        fd: c_int,
        family: c_int,
        type_: c_int,
        listening: c_int,
        port: u16,
    ) -> c_int;
    fn sd_is_socket_unix(
        fd: c_int,
        type_: c_int,
        listening: c_int,
        path: *const c_char,
        length: usize,
    ) -> c_int;
}

pub fn listen_fds(unset_environment: bool) -> io::Result<c_uint> {
    let r = unsafe { sd_listen_fds(if unset_environment { 1 } else { 0 }) };
    if r < 0 {
        return Err(io::Error::from_raw_os_error(-r));
    }
    Ok(r as c_uint)
}

pub fn is_socket_inet(fd: RawFd) -> io::Result<bool> {
    let r = unsafe { sd_is_socket_inet(fd, 0, 0, -1, 0) };
    if r < 0 {
        return Err(io::Error::from_raw_os_error(-r));
    }
    Ok(r != 0)
}

pub fn is_socket_unix(fd: RawFd) -> io::Result<bool> {
    let r = unsafe { sd_is_socket_unix(fd, 0, -1, null(), 0) };
    if r < 0 {
        return Err(io::Error::from_raw_os_error(-r));
    }
    Ok(r != 0)
}
