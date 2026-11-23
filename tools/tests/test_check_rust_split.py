# SPDX-License-Identifier: GPL-2.0-or-later
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
import runpy
import unittest


CHECKER = runpy.run_path(str(Path(__file__).resolve().parents[1] / 'check-rust-split.py'))


def report(old, new):
    output = StringIO()
    collect = CHECKER['collect']
    with redirect_stdout(output):
        failed = CHECKER['compare'](collect([('old.rs', old)]), collect([('new.rs', new)]))
    return failed, output.getvalue()


class RustSplitTests(unittest.TestCase):
    def test_handles_literals_comments_defaults_and_nested_functions(self):
        old = r'''
pub trait T {
    fn declaration();
    fn outer() {
        /* { /* nested */ } */
        fn inner() { let _ = (b'}', r#"fn false_positive() { "#); }
    }
}
'''
        new = old.replace('fn inner() {', 'pub(crate) fn inner () {').replace('        /* { /* nested */ } */', '')
        failed, output = report(old, new)
        self.assertTrue(failed)
        self.assertIn('changed trait T::outer:', output)
        self.assertIn('1 identical, 1 changed, 0 missing, 0 added', output)
        failed, output = report(old, new.replace('pub(crate) ', ''))
        self.assertFalse(failed)
        self.assertIn('2 identical, 0 changed, 0 missing, 0 added', output)

    def test_reports_changed_missing_and_added_bodies(self):
        old = 'fn same() {} fn update() { print!("two  spaces"); } fn removed() {}'
        new = 'pub fn same() { } fn update() { print!("two spaces"); } fn added() {}'
        failed, output = report(old, new)
        self.assertTrue(failed)
        self.assertIn('changed update:', output)
        self.assertIn('missing removed:', output)
        self.assertIn('added added:', output)

    def test_distinguishes_methods_with_the_same_name(self):
        old = 'impl A { fn f(&self) { a(); } } impl B { fn f(&self) { b(); } }'
        new = 'impl crate::B { fn f(&self) { a(); } } impl super::A { fn f(&self) { b(); } }'
        failed, output = report(old, new)
        self.assertTrue(failed)
        self.assertIn('changed impl A::f:', output)
        self.assertIn('changed impl B::f:', output)
