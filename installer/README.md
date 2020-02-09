# Windows installer

An Inno Setup Windows installer. 

## Icon

To add an icon to binary follow these steps:

- Install [rcedit](https://github.com/electron/rcedit),
- Add icon to binary: `rcedit-x86.exe "C:\path\to\release\hurban_selector.exe" --set-icon "C:\path\to\icons\64x64.ico"`

## Installer

To create a distribution follow these steps:

- Install [Inno Setup](http://www.jrsoftware.org/isinfo.php),
- `cargo build --release --features dist`,
- Compile `installer/setup.iss` from Inno Setup IDE,
- Output file will be located at `installer/bin`.

Installer itself will also contain [Microsoft Visual C++ Redistributable for
Visual Studio 2015, 2017 and 2019](https://support.microsoft.com/en-us/help/2977003/the-latest-supported-visual-c-downloads)
that is needed to run the application. Code used to install this dependency is
a modified version of [Inno Dependency Installer](https://github.com/domgho/innodependencyinstaller).

The only supported version is 64-bit and only English language is available.
