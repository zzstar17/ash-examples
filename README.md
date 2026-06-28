# Compute instance rendering

This example is still a work in progress.

`cargo run`

By default, it runs with all validation enabled. To run without validation, take look at `run_no_validation.sh` or just execute the script.

`sh run_no_validation.sh`

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.
- `log_alloc`: Log allocations in a friendly manner.

`vl`, `load` and `log_alloc` are enabled by default. To disable them, pass `--no-default-features` to cargo.
