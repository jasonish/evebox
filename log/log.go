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
	"fmt"
	"github.com/jasonish/evebox/exiter"
	"github.com/mattn/go-isatty"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

type LogLevel int

const (
	ERROR LogLevel = iota
	WARNING
	NOTICE
	INFO
	DEBUG
)

type Fields map[string]interface{}

var logLevel LogLevel = INFO

const (
	GREEN   = "\x1b[32m"
	BLUE    = "\x1b[34m"
	YELLOW  = "\x1b[33m"
	YELLOWB = "\x1b[1;33m"
	RED     = "\x1b[31m"
	//REDB    = "\x1b[1;31m"
	ORANGE = "\x1b[38;5;208m"
	RESET  = "\x1b[0m"
)

type ColorPrinter func(v interface{}) string

var Green ColorPrinter = noColorPrinter
var Blue ColorPrinter = noColorPrinter
var Red ColorPrinter = noColorPrinter
var Yellow ColorPrinter = noColorPrinter
var YellowB ColorPrinter = noColorPrinter
var Orange ColorPrinter = noColorPrinter

func init() {
	doColor := false
	if runtime.GOOS == "windows" {
		if isatty.IsCygwinTerminal(os.Stdout.Fd()) {
			doColor = true
		}
	} else if isatty.IsTerminal(os.Stdout.Fd()) {
		doColor = true
	}

	if doColor {
		// Register color printers.
		Green = _Green
		Blue = _Blue
		Yellow = _Yellow
		YellowB = _YellowB
		Red = _Red
		Orange = _Orange
	}
}

func noColorPrinter(v interface{}) string {
	return fmt.Sprintf("%v", v)
}

func _Green(v interface{}) string {
	return fmt.Sprintf("%s%v%s", GREEN, v, RESET)
}

func _Blue(v interface{}) string {
	return fmt.Sprintf("%s%v%s", BLUE, v, RESET)
}

func _Yellow(v interface{}) string {
	return fmt.Sprintf("%s%v%s", YELLOW, v, RESET)
}

func _YellowB(v interface{}) string {
	return fmt.Sprintf("%s%v%s", YELLOWB, v, RESET)
}

func _Red(v interface{}) string {
	return fmt.Sprintf("%s%v%s", RED, v, RESET)
}

func _Orange(v interface{}) string {
	return fmt.Sprintf("%s%v%s", ORANGE, v, RESET)
}

func Timestamp() string {
	now := time.Now()
	return now.Format("2006-01-02 15:04:05")
}

func SetLevel(level LogLevel) {
	logLevel = level
}

func GetLevel() LogLevel {
	return logLevel
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

	if level == WARNING {
		fmt.Fprintf(os.Stderr, "%s (%s:%s) <%s> -- %s\n",
			Green(Timestamp()),
			Blue(filepath.Base(filename)),
			Green(line),
			Orange("Warning"),
			Orange(fmt.Sprintf(format, v...)))
	}

	if level == NOTICE {
		fmt.Fprintf(os.Stderr, "%s (%s:%s) <%s> -- %s\n",
			Green(Timestamp()),
			Blue(filepath.Base(filename)),
			Green(line),
			YellowB("Notice"),
			fmt.Sprintf(format, v...))
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

func doLogWithFields(calldepth int, level LogLevel, fields Fields, format string, v ...interface{}) {
	if level > logLevel {
		return
	}
	msg := fmt.Sprintf(format, v...)
	doLog(calldepth, level, "%s -- %s", msg, formatFields(fields))
}

func Error(format string, v ...interface{}) {
	doLog(2, ERROR, format, v...)
}

func Warning(format string, v ...interface{}) {
	doLog(2, WARNING, format, v...)
}

func Notice(format string, v ...interface{}) {
	doLog(2, NOTICE, format, v...)
}

func Info(format string, v ...interface{}) {
	doLog(2, INFO, format, v...)
}

func InfoWithFields(fields Fields, format string, v ...interface{}) {
	doLogWithFields(3, INFO, fields, format, v...)
}

func formatFields(fields Fields) string {
	parts := []string{}
	for key, val := range fields {
		valstr := fmt.Sprintf("%v", val)
		if strings.Index(valstr, " ") > -1 {
			parts = append(parts,
				fmt.Sprintf("%s=\"%s\"", key, val))
		} else {
			parts = append(parts,
				fmt.Sprintf("%s=%s", key, val))
		}
	}
	return strings.Join(parts, " ")
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
	exiter.Exit(1)
}

func Fatalf(format string, v ...interface{}) {
	doLog(2, ERROR, format, v...)
	exiter.Exit(1)
}
