#! /bin/sh

rm -rf /Volumes/RAMDisk/pgdata
mkdir /Volumes/RAMDisk/pgdata
/opt/homebrew/Cellar/postgresql@15/15.3/bin/initdb -D /Volumes/RAMDisk/pgdata -U postgres --lc-collate=en_US.UTF-8 --lc-ctype=en_US.UTF-8 --locale=en_US.UTF-8
echo log_statement = 'all' >>/Volumes/RAMDisk/pgdata/postgresql.conf
export LC_ALL=en_US.UTF-8
/opt/homebrew/Cellar/postgresql@15/15.3/bin/postgres -D /Volumes/RAMDisk/pgdata
