use core::ops::ControlFlow;

use super::{PageDepth, TableEntry};
use crate::memory::Hhdm;
use libsys::table_index_size;

pub struct Walker<'a> {
    root_table: &'a [TableEntry],
    root_depth: PageDepth,
    target_depth: PageDepth,
}

impl<'a> Walker<'a> {
    /// ### Safety
    ///
    /// The provided page table must me a valid root-level table.
    pub unsafe fn new(table: &'a [TableEntry], depth: PageDepth, target_depth: PageDepth) -> Option<Self> {
        (depth >= target_depth).then_some(Self { root_table: table, root_depth: depth, target_depth })
    }

    pub fn walk<E>(&self, mut func: impl FnMut(Option<&TableEntry>) -> ControlFlow<E>) -> ControlFlow<E> {
        debug_assert!(self.root_depth > self.target_depth);

        Self::walk_impl(self.root_table, self.root_depth, self.target_depth, &mut func)
    }

    fn walk_impl<E>(
        table: &[TableEntry],
        cur_depth: PageDepth,
        target_depth: PageDepth,
        func: &mut impl FnMut(Option<&TableEntry>) -> ControlFlow<E>,
    ) -> ControlFlow<E> {
        if cur_depth == target_depth {
            table.iter().try_for_each(|entry| func(Some(entry)))?;
        } else if cur_depth > target_depth {
            for entry in table {
                if entry.is_present() {
                    let table = unsafe {
                        core::slice::from_raw_parts(
                            Hhdm::offset(entry.get_frame()).unwrap().as_ptr().cast(),
                            libsys::table_index_size(),
                        )
                    };

                    Self::walk_impl(table, cur_depth.next(), target_depth, func)?;
                } else {
                    debug!("{:?} >= {:?}", cur_depth, target_depth);
                    let steps = core::iter::Step::steps_between(&cur_depth, &target_depth).unwrap();
                    let iterations = table_index_size().pow(steps.try_into().unwrap());

                    (0..iterations).try_for_each(|_| func(None))?;
                }
            }
        }

        core::ops::ControlFlow::Continue(())
    }
}
