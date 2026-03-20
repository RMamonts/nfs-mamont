#!/usr/bin/env bash

dir=$1
num_files=$2
repeat=$3

sizes=(32 512 1K 2K 4K)

create() {
  for i in $(seq 1 "$1"); do
      touch "$dir/file_$i.txt"
  done
}

write() {
    for _ in $(seq 1 "$3"); do
      for i in $(seq 1 "$1"); do
          dd if=/dev/zero of="$dir/file_$i.txt" bs="$2" count=1 status=none
      done
    done
    echo "Write: $3 blocks of $2 into $1 files"
}

read_blocks() {
    for _ in $(seq 1 "$3"); do
      for i in $(seq 1 "$1"); do
          dd if="$dir/file_$i.txt" of=/dev/null bs="$2" count=1 status=none
      done
    done
    echo "Read: $3 blocks of $2 from $1 files"
}

clean() {
  for i in $(seq 1 "$1"); do
      rm -f "$dir/file_$i.txt"
  done
  echo "$1 tmp files deleted"
}

test_repeat_write_by_blocks() {
  create "$1"
  for size in "${sizes[@]}"; do
    write "$1" "$size" "$2"
  done
  clean "$1"
}

test_repeat_read_by_blocks() {
  create "$1"
  for i in $(seq 1 "$1"); do
      head -c 1M /dev/zero > "$dir/file_$i.txt"
  done

  for size in "${sizes[@]}"; do
    read_blocks "$1" "$size" "$2"
  done
  clean "$1"
}

test_readdir() {
  create "$1"

  echo "Readdir test ($2 repeats)"
  for _ in $(seq 1 "$2"); do
    ls -1 "$dir" > /dev/null
  done

  clean "$1"
}

test_readdir_deep() {
  create "$1"

  echo "Deep readdir test ($2 repeats)"
  for _ in $(seq 1 "$2"); do
    find "$dir" -type f > /dev/null
  done

  clean "$1"
}

test_create_nested_dirs() {
  depth=5
  width=10

  base="$dir/nested"
  rm -rf "$base"
  mkdir -p "$base"

  echo "Creating nested directories depth=$depth width=$width"

  parent="$base"
  for d in $(seq 1 $depth); do
    for w in $(seq 1 $width); do
      mkdir -p "$parent/dir_${d}_${w}"
    done
    parent="$parent/dir_${d}_1"
  done

  echo "Nested directory tree created"
  rm -rf "$base"
}

test_create_files_in_nested_dirs() {
  depth=4
  files_per_dir=20

  base="$dir/tree"
  rm -rf "$base"
  mkdir -p "$base"

  echo "Creating deep tree with files"

  parent="$base"
  for d in $(seq 1 $depth); do
    parent="$parent/level_$d"
    mkdir -p "$parent"

    for f in $(seq 1 $files_per_dir); do
      head -c 1024 /dev/zero > "$parent/file_${d}_${f}.txt"
    done
  done

  echo "Tree created"
  rm -rf "$base"
}

test_readdir_many_files() {
  count=5000
  mkdir -p "$dir/many"
  echo "Creating $count files…"

  for i in $(seq 1 $count); do
    touch "$dir/many/file_$i"
  done

  echo "Running readdir test"
  for _ in $(seq 1 "$1"); do
    ls "$dir/many" > /dev/null
  done

  rm -rf "$dir/many"
}

test_copy_by_blocks() {
  create "$1"

  for i in $(seq 1 "$1"); do
      head -c 1M /dev/urandom > "$dir/file_$i.txt"
  done

  echo "Copy test: $1 files, $2 repeats"

  for size in "${sizes[@]}"; do
    echo "Copying with block size $size"
    for _ in $(seq 1 "$2"); do
      for i in $(seq 1 "$1"); do
        dd if="$dir/file_$i.txt" of="$dir/file_$i.copy.txt" bs="$size" status=none
      done
    done

    for i in $(seq 1 "$1"); do
      rm -f "$dir/file_$i.copy.txt"
    done
  done

  clean "$1"
}



echo "=== WRITE TEST ==="
test_repeat_write_by_blocks "$num_files" "$repeat"

echo "=== READ TEST ==="
test_repeat_read_by_blocks "$num_files" "$repeat"

echo "=== READDIR TEST ==="
test_readdir "$num_files" "$repeat"

echo "=== DEEP READDIR TEST ==="
test_readdir_deep "$num_files" "$repeat"

echo "=== NESTED DIRS TEST ==="
test_create_nested_dirs

echo "=== NESTED DIRS WITH FILES TEST ==="
test_create_files_in_nested_dirs

echo "=== MANY FILES READDIR TEST ==="
test_readdir_many_files "$repeat"

echo "=== COPY TEST ==="
test_copy_by_blocks "$num_files" "$repeat"

