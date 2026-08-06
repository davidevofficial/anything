# Anything
Simple program made in rust with a GUI to find **any** file/directory in a list of drives. It works by first skimming the content of all selected drives and then creating an highly optimized index which later gets searched.

[Installation](https://github.com/davidevofficial/anything#Installation "installation")

<img width="1702" height="841" alt="Screenshot_20260806_112849" src="https://github.com/user-attachments/assets/8c074a16-3a6d-4319-9fc3-a15034f9b519" />

<img width="1702" height="841" alt="Screenshot_20260806_113134" src="https://github.com/user-attachments/assets/1a71b017-49bd-42b8-ab1f-ff87ff82ac8a" />


Supports:
- Supported Filesystems: ExFAT, Ext4, NTFS
- Indexing of drives
- Ignoring entries
- Sorting files
- Searches the full path or the file name
- Powerful search options
- Autodetects the Filesystems of drives
- Dynamically or periodically index drives
- Indexing is parallelized, making it blazingly fast 
- (Planned) Automatically add recently changed files without re-indexing

# Why?

When I had a windows machine I had [Everything](https://www.voidtools.com/downloads/ "Everything") (the tool from void tools) but when I switched to linux I found myself without a true alternative to Everything. I've tried countless tools and methods but all seem to be very slow, so I built myself this little tool.

I don't have a true benchmark but I tried dolphin (the file manager just to count files and dirs) and fsearch on my 1Tb ExFAT drive that contains 1 million files and they all took more than 30 minutes to index the drive while my little tool took 40 seconds.

Searching throught the index was on par with other tools.

# Installation

## Pre-built

Download pre-built binaries or the AppImage from the latest release.

Download FUSE if necessary:

For Ubuntu 22.04 LTS
```
sudo apt install libfuse2   
```
For Ubuntu 24.04 LTS
```
sudo apt install libfuse2t64 
```
Then
```
chmod +x Anything-version-x86_64.AppImage
```

Ready to use! see [how to use it](https://github.com/davidevofficial/anything?tab=readme-ov-file#how-to-use)

If you find any errors check ["Troubleshoot errors"](https://github.com/davidevofficial/anything#troubleshoot-errors)

## Build from source

```
git clone https://github.com/davidevofficial/anything.git
(cd inside where Cargo.toml is)
sudo apt install rustup
rustup default stable
(The following I believe are all necessary dependencies)
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
sudo apt-get install libatk1.0-dev libgdk-pixbuf2.0-dev
sudo apt-get install libgtk-3-dev
cargo build --release (or debug if you want debug)
```
or use the docker builder file

```
git clone https://github.com/davidevofficial/anything.git

(cd inside where Cargo.toml is)
cd anything

docker rm -f appimage-build 2>/dev/null
docker rmi rust-appimage-builder 2>/dev/null
docker build -t rust-appimage-builder . \
  && mkdir -p target \
  && docker run --rm -v "$PWD/target:/app/target" rust-appimage-builder \
  && find target -iname "*.AppImage"

```

**Don't forget the following commands:**
```
chmod +x Anything.AppImage
(and then to run)
sudo ./Anything.AppImage
```


Final File structure (after running for the first time) should look like this:
```
./
├── Anything
└── settings/
    ├── cache.txt
    ├── drives.txt
    ├── icon.png
    └── settings.txt
```

## Desktop Integration

To create a way to double click and run the AppImage you can generate a .desktop file
```
touch Anything.desktop
```

and write into the file (substitute /path/to/Anything.AppImage with the path to the AppImage). it should look something like this:
(remove "APPIMAGE_EXTRACT_AND_RUN=1" if FUSE is installed)
```
[Desktop Entry]
Comment=
Exec=pkexec env DISPLAY=$DISPLAY XAUTHORITY=$XAUTHORITY APPIMAGE_EXTRACT_AND_RUN=1 '/path/to/Anything.AppImage'
GenericName=file search GUI
Icon=/path/to/icon.png
Name=Anything
NoDisplay=false
Path=/path/to/directory/that/contains/anything
PrefersNonDefaultGPU=false
StartupNotify=true
Terminal=false
TerminalOptions=
Type=Application
X-KDE-SubstituteUID=false
X-KDE-Username=
```
Based on the Desktop Environment you can copy the file inside of /usr/share/applications/ or to /home/USER/.local/share/applications , doing so will add desktop integration

In the future I'll support other means for distributing the binary such as Flatpaks

# How to use

run with [sudo](https://github.com/davidevofficial/anything?tab=readme-ov-file#limitations "See limitation:") (sudo ./Anything.AppImage) for indexing

If indexing is not necessary you can run without sudo.

The main interface should be familiar to you if you come from windows (everything).

The bottom bar is a status bar, it tells you how many files it has found or if it is searching/indexing, errors, etc...

At the centre is a table containing five columns. Click any button on the column header to change sort mode. columns are also resizable.

The Top bar has three buttons and a search bar:

from left to right:

1. Settings Button
2. Index Button
3. Search Button
4. Search Bar

Click the Search button to search based on what you wrote in the search bar (if instant search is active it searches automatically 0.3 seconds after having finished typing)

Click the index button to read and index all files on all disks you selected.

The settings button opens a sub-menu with four buttons: Behaviour, Disks, Light mode and Help

## Behaviour

Index on startup: If it should automatically index when starting up the program

Dynamic Indexing: Whether to index based on how much time it took last time (for example: if it took 10 seconds, next index will be in 10 * dynamic_factor seconds)

Dynamic Factor: Determines how long to wait for dynamic index ( time_it_took_to_index_last_time * dynamic_factor )

Index Once every __ xyz __ minutes: Checks for changes after xyz minutes

Instant Search: Whether to click the search button to search (if not it automatically starts the search 0.3 seconds after you started typing and interrupts it when starting typing again)

Journal: I have yet to add this functionality

Ignore Case: Whether to ignore the case when searching for a file (for example if on xyz matches XyZ but also xyz or Xyz)

Search full path: If it searches the full path or just the file name

## Disks

Click the "+" button to start adding disks: that will open the lsblk window (select all drive you want to add), Click the "-" button to remove any drive

Click the ✏️ (pencil) button to edit ignored directories: to get started type inside of the square any path and press enter (for example you could type /.tmp, /root, /bin or /media/path_where/external_usb/mounted_at/.Trash-1000), to remove an ignored path just click the "-" button

When you are done press OK.


## Search Options

There are some options you can use to enhance your search to the next level, here is a simple guide on how to use those options!

### Simple Search

Just type anything in the top bar to start searching, if you type "xyz zyx" it searches all files which contain exactly "xyz zyx"

Example:
```
"important documents" -> Finds all elements that contain that substring
What gets searched: /home/important documents/ or /.tmp/important documents.pdf
What doesn't get searched: /home/important/documents  (will not find files or folders which include both or either word if it doesn't exactly contain that substring) 
```

### Complex Search

How to type a complex search query, the following are all valid:

\\(filter)

\\!(filter)

\\(filter)substring

\\!(filter)substring

Example:
```
\!(filter)substring -> The ! (negation) is optional, matches all files/folders that contain substring and gets filtered by the filter
\(size < 50kb)very_important_file -> finds all files which contain "very_important_file" and smaller than 50 kilobytes 
```

### Filters

Here is a list of all filters:
```
*(spaces inside the parenthesis don't matter)
*(After the parenthesis you can optionally add a substring)

\(size > x) or \(s > x) -> Size bigger than x
\(size < x) or \(s < x) -> Size bigger than x
Where x is either a number (like 1024) or a number followed by a prefix such as: b, k, m, g, t or b, kb, mb, gb, tb (case insensitive)

\(modified > DATE) or \(m > DATE) -> Modified after DATE
\(modified < DATE) or \(m < DATE) -> Modified before DATE
\(creation > DATE) or \(c > DATE) -> Created after DATE
\(creation < DATE) or \(c < DATE) -> Created before DATE
Where DATE is a date which can be written in different ways:
YYYY-mm-dd H:M:S
YYYY/mm/dd H:M:S
YYYY:mm:dd H:M:S
YYYY:mm:dd H-M-S
YYYY:mm:dd
YYYY/mm/dd
YYYY-mm-dd
(It follows the Year -> Month -> Date standard so be careful)

\(folder) -> Searches all folders
\(file) -> Searches all files
\(*.pdf) -> Ends with (in this example ".pdf")
\(run/media/usb_stick*) -> Starts with (in this example "run/media/usb_stick")

Example:
\(size < 1mb)\(/media/usb_stick*)\(created > 2025/1/1)important_file -> Finds "important_file", the path must start with /media/usb_stick, the file is smaller than 1mb and created after 2025/1/1
\(size > 5gb) -> Finds all files bigger than 5gb
\(folder)cache -> Finds all folders which contain the name "cache" inside (".cache" for example)
```

For each filter you can optionally add a ! to negate them or just write ! with no filter (making it the "doesn't contain" filter)

Examples:
```
\!name -> Doesn't contain "name"
\!(size > 1mb)name -> NOT bigger than 1mb and contains "name"
\!(m < 2020/1/2)\!name -> NOT modified before 2020/1/2 and NOT contains "name"
\(folder)\!name -> A folder which does NOT contain "name"
\(size < 1mb)\!(/media/1*)\(created > 2025/1/1)\!.cache\home -> Smaller than a mb, doesn't starts with /media/1, created after 2025/1/1, doesn't contain .cache and contains home
```


# Troubleshoot Errors

Q: WARNING: x drives do not exist or you do not have permission to open them

A: Means that one or more drives do not exist (incorrect path, drive was unmounted, bad drive) or that you ran the program without sudo and it attemps to open the drives with lacking permissions

Q: I think I found an error, bug or problem!

A: Send me an email at davidevufficial@gmail.com or open an issue here on github.

# Limitations

Anything requires sudo to index ( you can run the program without sudo to search and sort the files ) because it reads the /dev/sdXY drives directly.

In my case sudo is perfectly acceptable (especially because I made the program myself so I know it is not dangerous to run with sudo)

Also the index gets written to "settings/cache.txt" after quitting the program, my "settings/cache.txt" with 1 million files is 175mb so make sure you have free space.

# License

Copyright (c) Davidevofficial

This project is licensed under the GNU General Public License v3.0 (GPL-3.0).

Any contribution is appreciated <3.
