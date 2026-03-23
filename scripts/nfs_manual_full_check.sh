#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage:
  scripts/nfs_manual_full_check.sh <mount-point> [--keep]

Description:
  Runs an end-to-end functional check against an already mounted NFS export.
  The script creates a temporary test directory inside the mount point, exercises
  file, link, symlink, directory, rename, chmod and larger read/write flows,
  and removes everything on success.

Options:
  --keep   Keep the temporary test directory for inspection.
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
    usage
    exit 2
fi

mount_point=$1
keep_workdir=false
if [[ $# -eq 2 ]]; then
    if [[ $2 == "--keep" ]]; then
        keep_workdir=true
    else
        usage
        exit 2
    fi
fi

if [[ ! -d "$mount_point" ]]; then
    echo "error: mount point does not exist: $mount_point" >&2
    exit 1
fi

if ! mountpoint -q "$mount_point"; then
    echo "error: not a mount point: $mount_point" >&2
    exit 1
fi

workdir="$mount_point/.nfs-mamont-check-$$"
cleanup() {
    if [[ "$keep_workdir" == true ]]; then
        echo "kept test workspace: $workdir"
        return
    fi
    rm -rf -- "$workdir"
}
trap cleanup EXIT

pass() {
    echo "[ok] $1"
}

fail() {
    echo "[fail] $1" >&2
    exit 1
}

expect_file() {
    local path=$1
    [[ -f "$path" ]] || fail "expected regular file: $path"
}

expect_dir() {
    local path=$1
    [[ -d "$path" ]] || fail "expected directory: $path"
}

expect_symlink() {
    local path=$1
    [[ -L "$path" ]] || fail "expected symlink: $path"
}

expect_absent() {
    local path=$1
    [[ ! -e "$path" ]] || fail "expected path to be absent: $path"
}

expect_text() {
    local path=$1
    local expected=$2
    local actual
    actual=$(cat "$path")
    [[ "$actual" == "$expected" ]] || fail "unexpected contents in $path: '$actual'"
}

expect_same_inode() {
    local path_a=$1
    local path_b=$2
    local ino_a
    local ino_b
    ino_a=$(stat -c '%i' "$path_a")
    ino_b=$(stat -c '%i' "$path_b")
    [[ "$ino_a" == "$ino_b" ]] || fail "inode mismatch: $path_a vs $path_b"
}

expect_size() {
    local path=$1
    local expected=$2
    local actual
    actual=$(stat -c '%s' "$path")
    [[ "$actual" == "$expected" ]] || fail "unexpected size for $path: $actual"
}

expect_command_fail() {
    if "$@" >/dev/null 2>&1; then
        fail "command unexpectedly succeeded: $*"
    fi
}

echo "Running NFS functional check in $workdir"
mkdir -p "$workdir"
pass "workspace created"

# 1. Basic file lifecycle: create, read, overwrite, append, rename, truncate, delete.
printf 'hello' > "$workdir/file.txt"
expect_file "$workdir/file.txt"
expect_text "$workdir/file.txt" 'hello'
pass "file create/read"

printf ' world' >> "$workdir/file.txt"
expect_text "$workdir/file.txt" 'hello world'
pass "file append"

printf 'HELLO' > "$workdir/file.txt"
expect_text "$workdir/file.txt" 'HELLO'
pass "file overwrite"

mv "$workdir/file.txt" "$workdir/file-renamed.txt"
expect_absent "$workdir/file.txt"
expect_text "$workdir/file-renamed.txt" 'HELLO'
pass "file rename"

truncate -s 2 "$workdir/file-renamed.txt"
expect_text "$workdir/file-renamed.txt" 'HE'
pass "file truncate"

chmod 640 "$workdir/file-renamed.txt"
mode=$(stat -c '%a' "$workdir/file-renamed.txt")
[[ "$mode" == '640' ]] || fail "unexpected mode after chmod: $mode"
pass "file chmod"

expect_command_fail ln -s missing-target "$workdir/file-renamed.txt"
pass "duplicate symlink path rejected"

# 2. Hard links.
ln "$workdir/file-renamed.txt" "$workdir/file-hardlink.txt"
expect_file "$workdir/file-hardlink.txt"
expect_text "$workdir/file-hardlink.txt" 'HE'
ino_a=$(stat -c '%i' "$workdir/file-renamed.txt")
ino_b=$(stat -c '%i' "$workdir/file-hardlink.txt")
[[ "$ino_a" == "$ino_b" ]] || fail "hard link inode mismatch"
pass "hard link create/read"

printf '++' >> "$workdir/file-hardlink.txt"
expect_text "$workdir/file-renamed.txt" 'HE++'
pass "hard link writes affect original"

rm "$workdir/file-hardlink.txt"
expect_absent "$workdir/file-hardlink.txt"
expect_text "$workdir/file-renamed.txt" 'HE++'
pass "hard link delete keeps original"

# 3. Symbolic links to files.
ln -s file-renamed.txt "$workdir/file-symlink.txt"
expect_symlink "$workdir/file-symlink.txt"
[[ $(readlink "$workdir/file-symlink.txt") == 'file-renamed.txt' ]] || fail "bad file symlink target"
expect_text "$workdir/file-symlink.txt" 'HE++'
pass "file symlink create/read"

mv "$workdir/file-symlink.txt" "$workdir/file-symlink-renamed.txt"
expect_absent "$workdir/file-symlink.txt"
expect_symlink "$workdir/file-symlink-renamed.txt"
[[ $(readlink "$workdir/file-symlink-renamed.txt") == 'file-renamed.txt' ]] || fail "bad renamed file symlink target"
expect_text "$workdir/file-symlink-renamed.txt" 'HE++'
pass "file symlink rename"

# 4. Directories: create, nested create, rename, remove.
mkdir "$workdir/dir"
expect_command_fail mkdir "$workdir/dir"
pass "duplicate directory creation rejected"

mkdir -p "$workdir/dir/subdir"
expect_dir "$workdir/dir/subdir"
pass "directory create"

printf 'nested-data' > "$workdir/dir/subdir/note.txt"
expect_text "$workdir/dir/subdir/note.txt" 'nested-data'
pass "nested file create"

mv "$workdir/dir" "$workdir/dir-renamed"
expect_absent "$workdir/dir"
expect_dir "$workdir/dir-renamed/subdir"
expect_text "$workdir/dir-renamed/subdir/note.txt" 'nested-data'
pass "directory rename"

mkdir "$workdir/empty-dst"
mv -T "$workdir/dir-renamed" "$workdir/empty-dst"
expect_absent "$workdir/dir-renamed"
expect_dir "$workdir/empty-dst/subdir"
expect_text "$workdir/empty-dst/subdir/note.txt" 'nested-data'
mv -T "$workdir/empty-dst" "$workdir/dir-renamed"
pass "directory rename over empty directory"

# 5. Symbolic links to directories.
ln -s dir-renamed "$workdir/dir-link"
expect_symlink "$workdir/dir-link"
[[ $(readlink "$workdir/dir-link") == 'dir-renamed' ]] || fail "bad directory symlink target"
expect_text "$workdir/dir-link/subdir/note.txt" 'nested-data'
pass "directory symlink create/read"

# 6. Replace existing file contents through another path.
printf 'updated through symlink' > "$workdir/dir-link/subdir/note.txt"
expect_text "$workdir/dir-renamed/subdir/note.txt" 'updated through symlink'
pass "edit through directory symlink"

expect_command_fail rmdir "$workdir/file-renamed.txt"
pass "rmdir rejects regular file"

expect_command_fail ln "$workdir/dir-renamed" "$workdir/dir-hardlink"
pass "hard link to directory rejected"

# 7. Non-empty directory removal should fail.
expect_command_fail rmdir "$workdir/dir-renamed"
pass "non-empty directory removal rejected"

# 8. Larger file I/O.
dd if=/dev/urandom of="$workdir/blob.bin" bs=4096 count=16 status=none
cp "$workdir/blob.bin" "$workdir/blob-copy.bin"
cmp "$workdir/blob.bin" "$workdir/blob-copy.bin"
pass "large file write/read/compare"

dd if=/dev/zero of="$workdir/sparse.bin" bs=1 count=3 seek=8192 conv=notrunc status=none
expect_size "$workdir/sparse.bin" 8195
prefix_hex=$(od -An -tx1 -N4 "$workdir/sparse.bin" | tr -d ' \n')
[[ "$prefix_hex" == '00000000' ]] || fail "unexpected sparse prefix bytes: $prefix_hex"
tail_hex=$(tail -c 3 "$workdir/sparse.bin" | od -An -tx1 | tr -d ' \n')
[[ "$tail_hex" == '000000' ]] || fail "unexpected sparse tail bytes: $tail_hex"
pass "sparse write with offset"

# 9. Replace existing file atomically via mv.
printf 'src-data' > "$workdir/src.txt"
printf 'dst-data' > "$workdir/dst.txt"
mv -f "$workdir/src.txt" "$workdir/dst.txt"
expect_absent "$workdir/src.txt"
expect_text "$workdir/dst.txt" 'src-data'
pass "rename over existing file"

printf 'one' > "$workdir/replace-a.txt"
printf 'two' > "$workdir/replace-b.txt"
ln "$workdir/replace-a.txt" "$workdir/replace-a-link.txt"
mv -f "$workdir/replace-b.txt" "$workdir/replace-a.txt"
expect_text "$workdir/replace-a.txt" 'two'
expect_text "$workdir/replace-a-link.txt" 'one'
link_ino=$(stat -c '%i' "$workdir/replace-a-link.txt")
new_ino=$(stat -c '%i' "$workdir/replace-a.txt")
[[ "$link_ino" != "$new_ino" ]] || fail "rename replacement unexpectedly reused old hard-link inode"
pass "rename replacement does not rewrite old hard link inode"

expect_command_fail mv -T "$workdir/file-renamed.txt" "$workdir/dir-renamed"
pass "rename file over directory rejected"

mkdir "$workdir/empty-remove"
rmdir "$workdir/empty-remove"
expect_absent "$workdir/empty-remove"
pass "empty directory remove"

# 10. Cleanup paths explicitly to validate delete flows.
rm "$workdir/file-symlink-renamed.txt"
rm "$workdir/file-renamed.txt"
rm "$workdir/dst.txt"
rm "$workdir/blob.bin" "$workdir/blob-copy.bin"
rm "$workdir/sparse.bin"
rm "$workdir/replace-a.txt" "$workdir/replace-a-link.txt"
rm "$workdir/dir-link/subdir/note.txt"
rm "$workdir/dir-link"
rmdir "$workdir/dir-renamed/subdir"
rmdir "$workdir/dir-renamed"
expect_absent "$workdir/file-renamed.txt"
expect_absent "$workdir/dir-renamed"
pass "file and directory delete flows"

echo
echo "NFS functional check passed."