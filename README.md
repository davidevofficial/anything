# Anything
Simple program made in rust with a GUI to find **any** file/directory in a list of drives.

[Installation](https://github.com/davidevofficial/anything#Installation "installation")

<img width="1603" height="725" alt="image" src="https://github.com/user-attachments/assets/679978fe-26a0-4deb-9e76-c1c3a1b3111e" />

<img width="1606" height="733" alt="image" src="https://github.com/user-attachments/assets/1339f0c0-aa26-4139-af43-475d35495cdc" />



Supports:
- Supported Filesystems: ExFAT...(planning to add Ext4 support and other filesystems)
- Indexing of drives
- Ignoring entries
- Sorting files
- Searches the full path or the file name
- Powerful search options
- (Planned) Use of the journal on the root drive to check if anything changed and update the index accordingly

# Why?

When I had a windows machine I had [Everything](https://www.voidtools.com/downloads/ "Everything") (the tool from void tools) but when I switched to linux I found myself without a true alternative to Everything. I've tried countless tools and methods but all seem to be very slow, so I built myself this little tool.

I don't have a true benchmark but I tried dolphin (the file manager just to count files and dirs) and fsearch on my 1Tb ExFAT drive that contains 1 million files and they all took more than 30 minutes to index the drive while my little tool took 40 seconds.

Searching throught the index was on par with other tools.

# Installation

Download pre-built binaries or the AppImage from the latest release

or build it from source

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

To create a way to click and run the AppImage generate a file (Anything.sh)
```
touch Anything.sh
```

and write into the file the following (substitute /path/to/Anything.AppImage with the path to the AppImage)
```
#!/usr/bin/env bash
xhost +SI:localuser:root
pkexec env DISPLAY=$DISPLAY /media/SSD_ESTERNA/Davide/Apps/Apps/anything/Anything.AppImage
```

In the future I'll support other means for distributing the binary such as Flatpaks

# How to use

run with [sudo](https://github.com/davidevofficial/anything?tab=readme-ov-file#limitations "See limitation:") for indexing and if indexing is not necessary you can drop priviliges.

The main interface should be familiar to you if you come from windows (everything).

The bottom bar is a status bar, it tells you how many files it has found or if it is searching/indexing

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

## Beheviour

Index on startup: If it should automatically index when starting up the program

Index Once every __ xyz __ minutes: Checks for changes after xyz minutes

Instant Search: Whether to click the search button to search (if not it automatically starts the search 0.3 seconds after you started typing and interrupts it when starting typing again)

Journal: I have yet to add this functionality

Ignore Case: Whether to ignore the case when searching for a file (for example if on xyz matches XyZ but also xyz or Xyz)

Search full path: If it searches the full path or just the file name

## Disks

Click the + button to start adding disks: that will open the lsblk window (select all drive you want to add)

Click the - button to remove any drive, click the combobox that says ExFAT to change the filesystem type of the disk (it doesn't support automatic filesystem type recognition)

To modify the ignored directories of a disk open: drives.txt and type inside the square brackets

Example:
```
/dev/sdc1 /media/1 Exfat [/media/1/.Trash-1000, /media/1/useless_directory, /media/1/top_secret_data]
```
it is important that each entry is separated by a comma AND a space (", ").

## Search Options

There are some options you can use to enhance your search to the next level, each starts with the backslash ("\\")

Each "\\" defines the start of a predicate (if no \\ checks if file contains what you typed).

Predicates contain options:

\\! : Negation

\\  : Normal (the space is necessary)

\\_*: Starts with

\\*_: Ends with


Examples:
```
xyz yyy      -> Searches if file contains "xyz yyy"
\!xyz yyy    -> Searches if file doesn't contain "xyz yyy"
\!xyz\ yyy   -> Searches if file doesn't contain "xyz" AND contains "yyy" (spaces must be escaped to create a new predicate)
\_*xyz       -> Starts with "xyz"
\*_xyz       -> Ends with "xyz"
\!_*xyz      -> Doesn't ends with "xyz"
\!*_xyz      -> Doesn't starts with "xyz"
\*_xyz\ yyy  -> Ends with "xyz" AND contains "yyy"   
```

# Limitations

The strength of Anything is also its biggest weakness, Anything requires sudo to index ( you can run the program without sudo to search and sort the files ) because it reads the /dev/sdXY drives directly.

In my case sudo is perfectly acceptable (especially because I made the program myself so I know it is not dangerous to run with sudo)

Another big problem is that support for each Filesystem is limited (it has to be added manually) for example it currently only support ExFAT filesystems

Also the index gets written to cache.txt after quitting and my cache.txt with 1 million files is 175mb so make sure you have free space.

# License

Copyright (c) Davidevofficial

This project is licensed under the GNU General Public License v3.0 (GPL-3.0).

Any contribution is appreciated <3.
