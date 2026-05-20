// ... updated with better rar message

fn compress_with_rar(...) {
    let bin = find_binary("rar").context(
        "rar not found in PATH. Install the official RAR CLI from https://rarlab.com/download.htm\n\
         (Note: it is not redistributable, so pesto cannot bundle it.)",
    )?;
    // ...
}