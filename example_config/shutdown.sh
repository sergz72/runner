#! /bin/sh

mount_point_file=mount_point

if [ -f $mount_point_file ]
then
  hdiutil detach `cat $mount_point_file`
  rm $mount_point_file
fi
