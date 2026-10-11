fn main() {
    // Compiles the .slint UI files into Rust code at build time
    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");

    // Embeds the icon and the product details in the .exe, shown by Explorer and in
    // Properties → Details (what <ApplicationIcon> and <Product> do in the C# .csproj)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/icon.ico")
            .set("ProductName", "Digital Detox")
            .set("FileDescription", "Digital Detox")
            .set("CompanyName", "telcoray")
            .set("LegalCopyright", "Copyright (c) 2026 telcoray");
        resource.compile().expect("failed to embed the Windows resources");
    }
}
