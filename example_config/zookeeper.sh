#! /bin/sh

rm -rf /Volumes/RAMDisk/kafka
mkdir -p /Volumes/RAMDisk/kafka/zookeeper-data
mkdir -p /Volumes/RAMDisk/kafka/logs
./zookeeper-server-start.sh ../config/zookeeper.properties
