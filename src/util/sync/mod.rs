//! Much of the code within this module was taken from:
//!        https://github.com/zesterer/spin-rs.
//!
//! The work done by the community to maintain this extremely useful package is
//! significant, and it is an amazing useful project for getting started in
//! embedded or bare-metal development on Rust. Please visit the repository and
//! leave a star—or better—provide contributions to the project to further its
//! mission.
//!
//! This derivation was modified to suit usage within an operating system. The
//! primary derivating change is ensuring that code obtaining exclusive access
//! to the internal representations of the `T` data is uninterruptible. This
//! allows the types in this module to be used safely (read: without errant
//! deadlocks) within an interruptible operating system context.
//!
//! Below is the full license text of the project for attribution:
//!
//! The MIT License (MIT)
//!
//! Copyright (c) 2014 Mathijs van de Nes
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to
//! deal in the Software without restriction, including without limitation the
//! rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
//! sell copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in
//! all copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
//! IN THE SOFTWARE.

#![allow(clippy::disallowed_types)]

mod lazy;
pub use lazy::*;

mod mutex;
pub use mutex::*;

mod once;
pub use once::*;

mod rwlock;
pub use rwlock::*;
