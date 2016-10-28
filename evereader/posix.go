// +build !windows

package evereader

import (
	"encoding/json"
	"os"
	"syscall"
)

func GetSys(fileinfo os.FileInfo) interface{} {
	stat, ok := fileinfo.Sys().(*syscall.Stat_t)
	if !ok {
		return nil
	}
	return map[string]interface{}{
		"inode": stat.Ino,
	}
}

func SameSys(a interface{}, b interface{}) bool {

	aa, ok := a.(map[string]interface{})
	if !ok {
		return false
	}

	bb, ok := b.(map[string]interface{})
	if !ok {
		return false
	}

	var aaa uint64
	var bbb uint64

	switch t := aa["inode"].(type) {
	case json.Number:
		tmp, err := t.Int64()
		if err != nil {
			return false
		}
		aaa = uint64(tmp)
	case uint64:
		bbb = t
	case int64:
		bbb = uint64(t)
	default:
		return false
	}

	switch t := bb["inode"].(type) {
	case json.Number:
		tmp, err := t.Int64()
		if err != nil {
			return false
		}
		bbb = uint64(tmp)
	case uint64:
		bbb = t
	case int64:
		bbb = uint64(t)
	default:
		return false
	}

	if aaa != bbb {
		return false
	}

	return true
}
