# Copyright (C) 2024 Gramine contributors
# SPDX-License-Identifier: BSD-3-Clause

# PyTorch and the pre-trained model must be installed on the system. See README for details.

SHELL := /bin/bash

THIS_DIR := $(dir $(lastword $(MAKEFILE_LIST)))
VENV_DIR ?= $(THIS_DIR)/venv-gramine
ENTRYPOINT := $(VENV_DIR)/bin/python3

ARCH_LIBDIR ?= /lib/$(shell $(CC) -dumpmachine)

ifeq ($(DEBUG),1)
GRAMINE_LOG_LEVEL = debug
else
GRAMINE_LOG_LEVEL = error
endif

.PHONY: all
all: $(VENV_DIR)/.INSTALLATION_OK enclave.manifest
ifeq ($(SGX),1)
all: enclave.manifest.sgx enclave.sig
endif

.PRECIOUS: $(VENV_DIR)/.INSTALLATION_OK
$(VENV_DIR)/.INSTALLATION_OK:
	python3 -m venv $(VENV_DIR) \
	&& source $(VENV_DIR)/bin/activate \
	&& pip3 install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cpu \
	&& deactivate \
	&& touch $@

enclave.manifest: enclave.manifest.template $(VENV_DIR)/.INSTALLATION_OK
	gramine-manifest \
		-Dlog_level=$(GRAMINE_LOG_LEVEL) \
		-Darch_libdir=$(ARCH_LIBDIR) \
		-Dentrypoint=$(abspath $(ENTRYPOINT)) \
		-Dvenv_dir=$(abspath $(VENV_DIR)) \
		$< > $@

# Make on Ubuntu <= 20.04 doesn't support "Rules with Grouped Targets" (`&:`),
# for details on this workaround see
# https://github.com/gramineproject/gramine/blob/e8735ea06c/CI-Examples/helloworld/Makefile
enclave.manifest.sgx enclave.sig: sgx_sign
	@:

.INTERMEDIATE: sgx_sign
sgx_sign: enclave.manifest
	gramine-sgx-sign \
		--manifest $< \
		--output $<.sgx

.PHONY: clean
clean:
	$(RM) *.token *.sig *.manifest.sgx *.manifest

.PHONY: distclean
distclean: clean
	$(RM) -r *.pt result.txt $(VENV_DIR)
