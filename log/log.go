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

package log

import (
	"runtime"
	"fmt"
	"os"
	"time"
	"path/filepath"
)

type LogLevel int

const (
	ERROR LogLevel = iota
	INFO
	DEBUG
)

var logLevel LogLevel = INFO

const (
	GREEN = "\x1b[32m"
	BLUE = "\x1b[34m";
	REDB = "\x1b[1;31m";
	YELLOW = "\x1b[33m";
	RED = "\x1b[31m";
	YELLOWB = "\x1b[1;33m";
	RESET = "\x1b[0m"
)

func Green(v interface{}) string {
	return fmt.Sprintf("%s%v%s", GREEN, v, RESET)
}

func Blue(v interface{}) string {
	return fmt.Sprintf("%s%v%s", BLUE, v, RESET)
}

func Yellow(v interface{}) string {
	return fmt.Sprintf("%s%v%s", YELLOW, v, RESET)
}

func Red(v interface{}) string {
	return fmt.Sprintf("%s%v%s", RED, v, RESET)
}

func Timestamp() string {
	now := time.Now()
	return now.Format("2006-01-02 15:04:05")
}

func SetLevel(level LogLevel) {
	logLevel = level
}

func doLog(calldepth int, level LogLevel, format string, v ...interface{}) {

	if level > logLevel {
		return
	}

	_, filename, line, _ := runtime.Caller(calldepth)

	if level == ERROR {
		fmt.Fprintf(os.Stderr, "%s (%s:%s) <%s> -- %s\n",
			Green(Timestamp()),
			Blue(filepath.Base(filename)),
			Green(line),
			Red("Error"),
			Red(fmt.Sprintf(format, v...)))
	}

	if level == INFO {
		fmt.Fprintf(os.Stderr, "%s (%s:%s) <%s> -- %s\n",
			Green(Timestamp()),
			Blue(filepath.Base(filename)),
			Green(line),
			Blue("Info"),
			fmt.Sprintf(format, v...))
	}

	if level == DEBUG {
		fmt.Fprintf(os.Stderr, "%s (%s:%s) <%s> -- %s\n",
			Green(Timestamp()),
			Blue(filepath.Base(filename)),
			Green(line),
			Yellow("Debug"),
			fmt.Sprintf(format, v...))
	}
}

func Error(format string, v ...interface{}) {
	doLog(2, ERROR, format, v...)
}

func Info(format string, v ...interface{}) {
	doLog(2, INFO, format, v...)
}

func Debug(format string, v ...interface{}) {
	doLog(2, DEBUG, format, v...)
}

// Promote to info...
func Println(v ...interface{}) {
	doLog(2, INFO, "%s", fmt.Sprint(v...))
}

// To be compatible with standard logging, promote to info.
func Printf(format string, v ...interface{}) {
	doLog(2, INFO, format, v...)
}

func Fatal(v ...interface{}) {
	doLog(2, ERROR, "%s", fmt.Sprint(v...))
	os.Exit(1)
}