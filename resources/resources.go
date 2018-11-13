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

package resources

import (
	"bytes"
	"fmt"
	"github.com/gobuffalo/packr"
	"github.com/jasonish/evebox/log"
	"io"
	"io/ioutil"
	"os"
)

var ResourceBox packr.Box = packr.NewBox(".")

func GetEmbeddedAsset(name string) ([]byte, error) {
	bytes, err := ResourceBox.MustBytes(name)
	return bytes, err
}

func GetAsset(name string) ([]byte, error) {
	realName := fmt.Sprintf("./resources/%s", name)
	realAsset, err := os.Open(realName)
	if err == nil {
		log.Debug("Found on-disk asset for %s.", realName)
		return ioutil.ReadAll(realAsset)
	}
	return GetEmbeddedAsset(name)
}

// AssetString returns an asset as a string.
func AssetString(name string) (string, error) {
	bytes, err := GetAsset(name)
	if err != nil {
		return "", err
	}
	return string(bytes), nil
}

// GetReader returns an asset as a reader.
func GetReader(name string) (io.Reader, error) {
	data, err := GetAsset(name)
	if err != nil {
		return nil, err
	}
	return bytes.NewReader(data), nil
}
