# signal-desktop-image-extractor

Extract images from your local signal-desktop installation, with timestamps and using filenames and content type extensions where possible.

## Disclaimers

1. Your last backup is too damn old, make a new one _now_.
2. This program - while it might look cute - will eat your cat and more. See 1.
3. DO NOT IMPORT THE IMAGES EXPORTED BLINDLY INTO SOMETHING THAT USES THE EXIF METADATA FOR SORTING, IT WILL CAUSE HAVOC, see point 1.

## Backstory

As an avid signal user, I do have a lot of images stored in there. Recently, I lost all data of my Android phone and unfortunately had a
borked signal messages backup, so the signal desktop was the only store of all these images. The directories were unfortunately hard to
manouver, previews being mixed in with actual files, all with 64 bytes in length with no extension.

## Goals

I wanted a per conversation view of all images, using filenames of them where possible, sorted by date.
Signal removes the metadata of images as of today, which is inconvenient for my usecase, but the date of the image being send is hence the closest we can get of the image being taken.
