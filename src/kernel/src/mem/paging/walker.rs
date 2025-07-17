use core::ops::ControlFlow;

use crate::mem::HigherHalfDirectMap;

use super::{PageTableEntry, TableDepth};
use libsys::table_index_size;

pub struct Walker<'a> {
    root_table: &'a [PageTableEntry],
    root_depth: TableDepth,
    target_depth: TableDepth,
}

impl<'a> Walker<'a> {
    /// ## Safety
    ///
    /// The provided page table must me a valid root-level table.
    pub unsafe fn new(
        table: &'a [PageTableEntry],
        depth: TableDepth,
        target_depth: TableDepth,
    ) -> Option<Self> {
        (depth >= target_depth).then_some(Self {
            root_table: table,
            root_depth: depth,
            target_depth,
        })
    }

    pub fn walk<E>(
        &self,
        mut func: impl FnMut(Option<&PageTableEntry>) -> ControlFlow<E>,
    ) -> ControlFlow<E> {
        debug_assert!(self.root_depth > self.target_depth);

        Self::walk_impl(
            self.root_table,
            self.root_depth,
            self.target_depth,
            &mut func,
        )
    }

    fn walk_impl<E>(
        table: &[PageTableEntry],
        cur_depth: TableDepth,
        target_depth: TableDepth,
        func: &mut impl FnMut(Option<&PageTableEntry>) -> ControlFlow<E>,
    ) -> ControlFlow<E> {
        use core::cmp::Ordering;

        match cur_depth.cmp(&target_depth) {
            Ordering::Equal => table.iter().try_for_each(|entry| func(Some(entry)))?,

            Ordering::Greater => {
                for entry in table {
                    if entry.is_present() {
                        let table_ptr = core::ptr::with_exposed_provenance_mut(
                            HigherHalfDirectMap::frame_to_page(entry.get_frame())
                                .get()
                                .get(),
                        );

                        // Safety: If the table entry is present, then the table is guaranteed to
                        //         exist, and be valid for `[PageTableEntry; table_index_size()]`.
                        let table = unsafe {
                            core::slice::from_raw_parts(table_ptr, libsys::table_index_size())
                        };

                        Self::walk_impl(table, cur_depth.next(), target_depth, func)?;
                    } else {
                        let (steps, _) = core::iter::Step::steps_between(&cur_depth, &target_depth);
                        let iterations = table_index_size().pow(steps.try_into().unwrap());
                        (0..iterations).try_for_each(|_| func(None))?;
                    }
                }
            }

            Ordering::Less => unreachable!(),
        }

        core::ops::ControlFlow::Continue(())
    }
}
