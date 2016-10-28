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
	"bufio"
	"bytes"
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"io"
	"os"
)

type EveReader struct {
	path   string
	file   *os.File
	reader *bufio.Reader
	lineno uint64
	size   int64
}

func New(path string) (*EveReader, error) {

	eveReader := EveReader{
		path: path,
	}
	err := eveReader.OpenFile()
	if err != nil {
		return nil, err
	}

	return &eveReader, nil
}

func (er *EveReader) GetFileInfo() (os.FileInfo, error) {
	return er.file.Stat()
}

func (er *EveReader) OpenFile() error {
	file, err := os.Open(er.path)
	if err != nil {
		return err
	}

	fileInfo, err := file.Stat()
	if err != nil {
		return err
	}

	er.file = file
	er.reader = bufio.NewReader(er.file)
	er.size = fileInfo.Size()
	er.lineno = 0

	return nil
}

func (er *EveReader) Close() {
	er.file.Close()
}

func (er *EveReader) Reopen() error {
	log.Debug("Reopening %s", er.path)
	er.file.Close()
	return er.OpenFile()
}

// Skip to a line number in the file. Must be called before any reading is
// done.
func (er *EveReader) SkipTo(lineno uint64) error {
	if er.lineno != 0 {
		return nil
	}
	for lineno > 0 {
		_, err := er.reader.ReadBytes('\n')
		if err != nil {
			return err
		}
		lineno--
		er.lineno++
	}
	return nil
}

func (er *EveReader) SkipToEnd() error {

	for {
		_, err := er.reader.ReadBytes('\n')
		if err != nil {
			if err == io.EOF {
				break
			} else {
				return err
			}
		} else {
			er.lineno++
		}
	}

	return nil
}

// Get the current position in the file. For EveReaders this is the line number
// as the actual file offset is not useful due to buffering in the json
// decoder as well as bufio.
func (er *EveReader) Pos() uint64 {
	return er.lineno
}

func (er *EveReader) FileOffset() (int64, error) {
	return er.file.Seek(0, 1)
}

func (er *EveReader) FileSize() (int64, error) {
	info, err := er.file.Stat()
	if err != nil {
		return 0, err
	}
	return info.Size(), nil
}

func (er *EveReader) IsNewFile() bool {
	fileInfo1, err := er.file.Stat()
	if err != nil {
		return false
	}
	fileInfo2, err := os.Stat(er.path)
	if err != nil {
		return false
	}
	return !os.SameFile(fileInfo1, fileInfo2)
}

func (er *EveReader) Next() (eve.RawEveEvent, error) {

	// Check for file truncation.
	fileInfo, err := er.file.Stat()
	if err != nil {
		return nil, err
	}
	if fileInfo.Size() < er.size {
		// Truncated, seek to 0.
		er.file.Seek(0, 0)
		er.lineno = 0
	}
	er.size = fileInfo.Size()

	var event eve.RawEveEvent

	line, err := er.reader.ReadBytes('\n')
	if err != nil {
		if err == io.EOF {
			// Check for rotation.
			if er.IsNewFile() {
				err = er.Reopen()
				if err != nil {
					return nil, err
				}
				return er.Next()
			}
		}
		return nil, err
	}

	er.lineno++

	decoder := json.NewDecoder(bytes.NewReader(line))
	decoder.UseNumber()

	if err := decoder.Decode(&event); err != nil {
		return nil, err
	}

	return event, nil
}
