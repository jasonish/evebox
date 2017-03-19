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
	"fmt"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"io"
	"os"
)

type MalformedEventError struct {
	Event  string
	Err    error
	LineNo uint64
}

func (e MalformedEventError) Error() string {
	return fmt.Sprintf("Failed to parse event: %s: %s", e.Err.Error(), e.Event)
}

// FollowingReader is an EveReader built on top of the basic reader that
// handles continuous (following) type reading that can handle rotation,
// either truncation, or file rename.
type FollowingReader struct {
	*BasicReader

	path   string
	lineno uint64
	size   int64
}

func NewFollowingReader(path string) (*FollowingReader, error) {
	eveReader := FollowingReader{
		path: path,
	}
	if err := eveReader.OpenFile(); err != nil {
		return nil, err
	}

	return &eveReader, nil
}

func (r *FollowingReader) GetFileInfo() (os.FileInfo, error) {
	return r.file.Stat()
}

func (r *FollowingReader) OpenFile() error {
	br, err := NewBasicReader(r.path)
	if err != nil {
		return err
	}
	r.BasicReader = br
	r.lineno = 0

	return nil
}

func (r *FollowingReader) Reopen() error {
	log.Debug("Reopening %s", r.path)
	r.BasicReader.Close()
	return r.OpenFile()
}

// Skip to a line number in the file. Must be called before any reading is
// done.
func (r *FollowingReader) SkipTo(lineno uint64) error {
	if r.lineno != 0 {
		return nil
	}
	for lineno > 0 {
		_, err := r.BasicReader.NextLine()
		if err != nil {
			return err
		}
		lineno--
		r.lineno++
	}
	return nil
}

func (r *FollowingReader) SkipToEnd() error {
	for {
		_, err := r.BasicReader.NextLine()
		if err != nil {
			if err == io.EOF {
				break
			} else {
				return err
			}
		} else {
			r.lineno++
		}
	}

	return nil
}

// Get the current position in the file. For EveReaders this is the line number
// as the actual file offset is not useful due to buffering in the json
// decoder as well as bufio.
func (r *FollowingReader) Pos() uint64 {
	return r.lineno
}

func (r *FollowingReader) IsNewFile() bool {
	fileInfo1, err := r.BasicReader.Stat()
	if err != nil {
		return false
	}
	fileInfo2, err := os.Stat(r.path)
	if err != nil {
		return false
	}
	return !os.SameFile(fileInfo1, fileInfo2)
}

func (r *FollowingReader) Next() (eve.EveEvent, error) {

	// Check for file truncation.
	size, err := r.FileSize()
	if err != nil {
		return nil, err
	}
	if size < r.size {
		// Truncated, seek to 0.
		r.BasicReader.SetOffset(0)
		r.lineno = 0
	}
	r.size = size

	line, err := r.NextLine()

	if err != nil {
		if err == io.EOF {
			// Check for rotation.
			if r.IsNewFile() {
				err = r.Reopen()
				if err != nil {
					return nil, err
				}
				return r.Next()
			}
		}
		return nil, err
	}

	event, err := eve.NewEveEventFromBytes(line)
	if err != nil {
		return nil, MalformedEventError{
			Event:  string(line),
			Err:    err,
			LineNo: r.lineno,
		}
	}

	r.lineno++

	return event, nil
}

// Return the "lag" in bytes, that is the number of bytes behind the reader is
// from the end of the file.
func (r *FollowingReader) Lag() (int64, error) {
	fileSize, err := r.FileSize()
	if err != nil {
		return 0, err
	}
	fileOffset, err := r.FileOffset()
	if err != nil {
		return 0, err
	}
	return fileSize - fileOffset, nil
}
