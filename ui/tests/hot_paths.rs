// SPDX-License-Identifier: GPL-2.0-or-later

//! Process isolation keeps unrelated test threads out of the allocation census.

mod support;

#[global_allocator]
static HEAP: support::counting::Counting = support::counting::Counting::new();

#[test]
fn hot_paths_allocate_nothing_after_warmup() {
    if std::env::var_os("OOS_ALLOC_CHILD").is_none() {
        let inputs = support::isolated_inputs();
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "hot_paths_allocate_nothing_after_warmup", "--nocapture"]).env("OOS_ALLOC_CHILD", "1");
        let output = support::run_command(&inputs, command, 120);
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
        return;
    }
    // Prove the observer sees freed transients, zeroed allocations and reallocations.
    let (count, ()) = HEAP.count(|| {
        let mut v = Vec::<u8>::with_capacity(16);
        v.push(1);
        v.reserve_exact(4096);
        std::hint::black_box(v);
        std::hint::black_box(vec![0u8; 4096]);
    });
    assert_eq!(count, 3, "alloc, realloc and alloc_zeroed are all counted after free");
    let mut paths = 0;
    oos_ui::Ui::hot_path_probe(|name, frame| {
        for _ in 0..8 {
            frame();
        }
        for index in 0..64 {
            let (allocations, ()) = HEAP.count(&mut *frame);
            assert_eq!(allocations, 0, "{name}: post-warmup frame {index}");
        }
        paths += 1;
        println!("ALLOC_BUDGET {} 64 0", name.replace(' ', "_"));
    });
    assert_eq!(paths, 4, "all currently guarded UI hot kernels were exercised");
}
