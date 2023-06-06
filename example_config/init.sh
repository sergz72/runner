#! /bin/sh

mount_point_file=mount_point

if [ ! -f $mount_point_file ]
then
  mount_point=`hdiutil attach -nobrowse -nomount ram://8388608`
  echo $mount_point >$mount_point_file
  diskutil erasevolume HFS+ RAMDisk $mount_point
fi
