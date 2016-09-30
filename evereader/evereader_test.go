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
	"io"
	"log"
	"os"
	"testing"
)

var rawEvent string = `{"timestamp":"2016-09-15T11:23:20.197956-0600","flow_id":943590776193120,"event_type":"alert","src_ip":"82.165.177.154","src_port":80,"dest_ip":"10.16.1.11","dest_port":59852,"proto":"TCP","http":{"hostname":"www.testmyids.com","url":"\/","http_user_agent":"curl\/7.47.1","http_content_type":"text\/html","http_method":"GET","protocol":"HTTP\/1.1","status":200,"length":39},"payload":"SFRUUC8xLjEgMjAwIE9LDQpEYXRlOiBUaHUsIDE1IFNlcCAyMDE2IDE3OjIzOjIwIEdNVA0KU2VydmVyOiBBcGFjaGUNCkxhc3QtTW9kaWZpZWQ6IE1vbiwgMTUgSmFuIDIwMDcgMjM6MTE6NTUgR01UDQpFVGFnOiAiMjctNDI3MWM1ZjFhYzRjMCINCkFjY2VwdC1SYW5nZXM6IGJ5dGVzDQpDb250ZW50LUxlbmd0aDogMzkNCkNvbnRlbnQtVHlwZTogdGV4dC9odG1sDQoNCnVpZD0wKHJvb3QpIGdpZD0wKHJvb3QpIGdyb3Vwcz0wKHJvb3QpCg==","payload_printable":"HTTP\/1.1 200 OK\r\nDate: Thu, 15 Sep 2016 17:23:20 GMT\r\nServer: Apache\r\nLast-Modified: Mon, 15 Jan 2007 23:11:55 GMT\r\nETag: \"27-4271c5f1ac4c0\"\r\nAccept-Ranges: bytes\r\nContent-Length: 39\r\nContent-Type: text\/html\r\n\r\nuid=0(root) gid=0(root) groups=0(root)\n","stream":1,"packet":"RQABLhOhQAAyBiTPUqWxmgoQAQsAUOnMrUcvtFca6JaAGAFU0FAAAAEBCAoXuzwUD2A3JkhUVFAvMS4xIDIwMCBPSw0KRGF0ZTogVGh1LCAxNSBTZXAgMjAxNiAxNzoyMzoyMCBHTVQNClNlcnZlcjogQXBhY2hlDQpMYXN0LU1vZGlmaWVkOiBNb24sIDE1IEphbiAyMDA3IDIzOjExOjU1IEdNVA0KRVRhZzogIjI3LTQyNzFjNWYxYWM0YzAiDQpBY2NlcHQtUmFuZ2VzOiBieXRlcw0KQ29udGVudC1MZW5ndGg6IDM5DQpDb250ZW50LVR5cGU6IHRleHQvaHRtbA0KDQp1aWQ9MChyb290KSBnaWQ9MChyb290KSBncm91cHM9MChyb290KQo=","packet_info":{"linktype":12},"host":"fw","alert":{"action":"allowed","gid":1,"signature_id":10000000,"rev":1,"signature":"","category":"Potentially Bad Traffic","severity":2}}`

type TestEveWriter struct {
	filename string
	file     *os.File
}

func OpenTestEveWriter(filename string) (*TestEveWriter, error) {

	file, err := os.Create(filename)
	if err != nil {
		return nil, err
	}

	return &TestEveWriter{filename: filename, file: file}, nil
}

func (w *TestEveWriter) WriteLine(line string) {
	w.file.WriteString(line)
	w.file.WriteString("\n")
	w.file.Sync()
}

func (w *TestEveWriter) Truncate() {

	if err := w.file.Truncate(0); err != nil {
		log.Fatal(err)
	}
	w.file.Seek(0, 0)
}

func (w *TestEveWriter) Close() {
	w.file.Close()
}

func TestEveReaderFollow(t *testing.T) {

	filename := "TestEveReaderFollow.test.json"
	writer, err := OpenTestEveWriter(filename)
	if err != nil {
		t.Fatal(err)
	}
	defer writer.Close()
	defer os.Remove(filename)

	reader, err := New(filename)
	if err != nil {
		t.Fatal(err)
	}

	// Expect EOF.
	_, err = reader.Next()
	if err == nil {
		t.Fatal("err shold not be nil")
	} else if err != io.EOF {
		t.Fatal("expected err to be io.EOF")
	}

	for i := 0; i < 10; i++ {

		// Write out a single event.
		writer.WriteLine(rawEvent)

		event, err := reader.Next()
		if err != nil {
			t.Fatal(err)
		}
		if event == nil {
			t.Fatal("event should not be nil")
		}
	}

	// Now should get an EOF.
	_, err = reader.Next()
	if err == nil || err != io.EOF {
		t.Fatal("expected err to be io.EOF")
	}
}

// Test the reading of a log file that was truncated (like logrotates
// copytruncate option).
func TestEveReaderFollowTruncate(t *testing.T) {

	filename := "TestEveReaderFollowTruncate.test.json"
	writer, err := OpenTestEveWriter(filename)
	if err != nil {
		t.Fatal(err)
	}
	defer writer.Close()
	defer os.Remove(filename)

	reader, err := New(filename)
	if err != nil {
		t.Fatal(err)
	}

	// Write out a single event.
	writer.WriteLine(rawEvent)

	// Get an event.
	_, err = reader.Next()
	if err != nil {
		t.Fatal(err)
	}

	if reader.Pos() != 1 {
		t.Fatal("expected position of 1")
	}

	// Truncate, should read EOF.
	writer.Truncate()
	_, err = reader.Next()
	if err == nil || err != io.EOF {
		t.Fatal("expected eof")
	}

	// Write another event.
	writer.WriteLine(rawEvent)

	// Should read an event.
	_, err = reader.Next()
	if err != nil {
		t.Fatal(err)
	}

	// As the file was truncated, position should be at one again.
	if reader.Pos() != 1 {
		t.Fatal("expected position of 1")
	}
}

// Test the reading of a log file that is renamed then re-opened.
func TestEveReaderFollowRename(t *testing.T) {

	filename := "TestEveReaderFollowRename.test.json"

	writer, err := OpenTestEveWriter(filename)
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(filename)

	reader, err := New(filename)
	if err != nil {
		t.Fatal(err)
	}

	// Write an event; read an event.

	writer.WriteLine(rawEvent)

	_, err = reader.Next()
	if err != nil {
		t.Fatal(err)
	}

	// Close the writer and rename the file.
	writer.Close()
	os.Rename(filename, fmt.Sprintf("%s.1", filename))
	defer os.Remove(fmt.Sprintf("%s.1", filename))

	// Read again, should just get an EOF.
	_, err = reader.Next()
	if err == nil || err != io.EOF {
		t.Fatal("expected EOF")
	}

	// Open a new writer, same filename and write an event.
	writer, err = OpenTestEveWriter(filename)
	if err != nil {
		t.Fatal(err)
	}
	writer.WriteLine(rawEvent)

	// Read again, should get event from new file with position reset.
	_, err = reader.Next()
	if err != nil {
		t.Fatal(err)
	}
}

func TestEveReader_SkipToEnd(t *testing.T) {
	filename := "TestEveReader_SkipToEnd.json"
	defer os.Remove(filename)

	// Write out 100 events.
	writer, err := OpenTestEveWriter(filename)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 100; i++ {
		writer.WriteLine(rawEvent)
	}

	// Create a reader and skip to the end.
	reader, err := New(filename)
	if err != nil {
		t.Fatal(err)
	}
	if err := reader.SkipToEnd(); err != nil {
		t.Fatal(err)
	}
	lineno := reader.Pos()
	if lineno != 100 {
		t.Fatalf("Line number is %v; expected 100.", lineno)
	}

	// Should also get nil on read.
	event, err := reader.Next()
	if err == nil || err != io.EOF {
		t.Fatal("expected EOF")
	}
	if event != nil {
		t.Fatal("expected nil event")
	}

	// Write out an event and read it.
	writer.WriteLine(rawEvent)
	event, err = reader.Next()
	if err != nil {
		t.Fatal(err)
	}
	if event == nil {
		t.Fatal("got nil event")
	}
}