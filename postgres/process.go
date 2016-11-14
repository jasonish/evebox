package postgres

import (
	"os/exec"
	"io/ioutil"
	"strings"
	"bufio"
	"github.com/jasonish/evebox/log"
	"io"
	"path/filepath"
	"syscall"
)

func GetVersion() (string, error) {
	command := exec.Command("postgres", "--version")
	stdout, err := command.StdoutPipe()
	if err != nil {
		return "", err
	}
	err = command.Start()
	if err != nil {
		return "", err
	}
	output, err := ioutil.ReadAll(stdout)
	if err != nil {
		return "", err
	}
	versionString := string(output)
	command.Wait()
	return strings.TrimSpace(versionString), nil
}

func Init(directory string) error {
	command := exec.Command("initdb",
		"-D", directory,
		"--encoding=UTF8")

	stdout, err := command.StdoutPipe()
	if err != nil {
		log.Error("Failed to open initdb stdout, will not be logged.")
		stdout = nil
	}

	stderr, err := command.StderrPipe()
	if err != nil {
		log.Error("Failed to open initdb stderr, will not be logged.")
		stderr = nil
	}

	err = command.Start()
	if err != nil {
		log.Error("Failed to start initdb: %v", err)
		return err
	}

	if stdout != nil {
		go func() {
			if err := ReadPipe(stdout, true, "initdb stdout"); err != nil {
				log.Error("Failed to read from stdout: %v", err)
			}
		}()
	}

	if stderr != nil {
		go func() {
			if err := ReadPipe(stderr, true, "initdb stderr"); err != nil {
				log.Error("Failed to read from stderr: %v", err)
			}
		}()
	}

	return command.Wait()
}

func ReadPipe(pipe io.ReadCloser, doLog bool, logPrefix string) (error) {
	reader := bufio.NewReader(pipe)
	for {
		line, err := reader.ReadBytes('\n')
		if err != nil && err == io.EOF {
			break
		} else if err != nil {
			return err
		}
		log.Info("%s: %s", logPrefix, strings.TrimSpace(string(line)))
	}
	return nil
}

func Start(directory string) (*exec.Cmd, error) {

	// Get the absolute path if the data directory.
	path, err := filepath.Abs(directory)
	if err != nil {
		return nil, err
	}
	log.Info("Using postgres data directory %s", path)

	command := exec.Command("postgres",
		"-D", path,
		"-c", "log_destination=stderr",
		"-c", "logging_collector=off",
		"-k", path)

	stdout, err := command.StdoutPipe()
	if err != nil {
		log.Error("Failed to open postgres stdout, will not be logged.")
		stdout = nil
	}

	stderr, err := command.StderrPipe()
	if err != nil {
		log.Error("Failed to open postgres stderr, will not be logged.")
		stderr = nil
	}

	err = command.Start()
	if err != nil {
		log.Error("Failed to start postgres: %v", err)
		return nil, err
	}

	if stdout != nil {
		go func() {
			if err := ReadPipe(stdout, true, "postgres stdout"); err != nil {
				log.Error("Failed to read postgres stdout: %v", err)
			}
		}()
	}

	if stderr != nil {
		go func() {
			if err := ReadPipe(stderr, true, "postgres stderr"); err != nil {
				log.Error("Failed to read postgres stderr: %v", err)
			}
		}()
	}

	return command, nil
}

func Stop(command *exec.Cmd) {
	err := command.Process.Signal(syscall.SIGTERM)
	if err != nil {
		log.Error("Failed to stop postgres: %v", err)
	}
}