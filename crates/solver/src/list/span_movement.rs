//! Ownership-preserving span edits after coordinate validation.
use super::ListRange;

pub(super) fn relocate<V>(
    row: &mut [V],
    start: usize,
    end: usize,
    destination: usize,
    reverse: bool,
) {
    let width = end - start;
    if destination < start {
        row[destination..end].rotate_right(width);
    } else {
        row[start..destination + width].rotate_left(width);
    }
    if reverse {
        row[destination..destination + width].reverse();
    }
}

pub(super) fn exchange<V>(rows: &mut [Vec<V>], left: ListRange, right: ListRange, reverse: bool) {
    if left.entity == right.entity {
        let (first, last) = if left.start < right.start {
            (left, right)
        } else {
            (right, left)
        };
        let a = first.end - first.start;
        let b = last.end - last.start;
        let row = &mut rows[first.entity];
        // A, middle, B -> middle, B, A -> B, middle, A.
        row[first.start..last.end].rotate_left(a);
        row[first.start..last.end - a].rotate_right(b);
        if reverse {
            row[first.start..first.start + b].reverse();
            row[last.end - a..last.end].reverse();
        }
        return;
    }
    let (a, b) = if left.entity < right.entity {
        let (low, high) = rows.split_at_mut(right.entity);
        (&mut low[left.entity], &mut high[0])
    } else {
        let (low, high) = rows.split_at_mut(left.entity);
        (&mut high[0], &mut low[right.entity])
    };
    let aw = left.end - left.start;
    let bw = right.end - right.start;
    let common = aw.min(bw);
    a[left.start..left.start + common].swap_with_slice(&mut b[right.start..right.start + common]);
    // Only the unequal tail needs temporary ownership; equal-width swaps allocate nothing.
    if aw > bw {
        let tail: Vec<_> = a.drain(left.start + common..left.end).collect();
        b.splice(right.end..right.end, tail);
    } else if bw > aw {
        let tail: Vec<_> = b.drain(right.start + common..right.end).collect();
        a.splice(left.end..left.end, tail);
    }
    if reverse {
        a[left.start..left.start + bw].reverse();
        b[right.start..right.start + aw].reverse();
    }
}
