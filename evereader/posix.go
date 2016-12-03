// +build !windows

/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

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
