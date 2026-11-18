fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // gpui-pre-windows 以资源 ID 1 加载 exe 图标作为窗口类图标,
    // 因此这里必须把图标嵌为 ID 1(任务栏/标题栏图标来源)。
    let res_dir = std::path::Path::new("assets/icons");
    println!("cargo:rerun-if-changed=assets/icons/app.rc");
    println!("cargo:rerun-if-changed=assets/icons/icon.ico");
    embed_resource::compile(res_dir.join("app.rc"), embed_resource::ParamsIncludeDirs([res_dir]));
}