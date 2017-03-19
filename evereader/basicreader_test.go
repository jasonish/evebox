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
	"github.com/stretchr/testify/assert"
	"io"
	"os"
	"testing"
)

func TestBasicReader_PartialRead(t *testing.T) {

	filename := "TestEveReader_PartialRead.json"
	defer os.Remove(filename)

	writer, err := NewFileWriter(filename)
	assert.Nil(t, err)
	defer writer.Close()

	// Start by writing out a complete event.
	writer.WriteLine(rawEvent)

	// Now get a reader and read in the first event.
	reader, err := NewBasicReader(filename)
	assert.Nil(t, err)

	event, err := reader.Next()
	assert.Nil(t, err)
	assert.NotNil(t, event)
	defer reader.Close()

	// Write out a partial event, then the remainder of it and read.
	rawEventLen := len(rawEvent)
	bytesToWrite := rawEventLen / 2
	writer.Write(rawEvent[0:bytesToWrite])
	writer.WriteLine(rawEvent[bytesToWrite:])

	event, err = reader.Next()
	assert.Nil(t, err)
	assert.NotNil(t, event)

	// Ok, now write out the partial event and read.
	writer.Write(rawEvent[0:bytesToWrite])

	event, err = reader.Next()
	assert.Equal(t, io.EOF, err)
	assert.Nil(t, event)

	// Write out the rest of the event and read.
	writer.WriteLine(rawEvent[bytesToWrite:])

	event, err = reader.Next()
	assert.Nil(t, err)
	assert.NotNil(t, event)
}
