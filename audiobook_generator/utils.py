# Copyright 2025 Carlo Piras
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# -*- coding: utf-8 -*-
"""
This module provides various utility functions for the Audiobook Generator project,
such as text processing and filename sanitization.
"""

import re
from typing import Any

def count_words_proxy(text: str) -> int:
    """
    Provides a simple estimation of the word count in a given text.

    Args:
        text: The input string to count words from.

    Returns:
        The estimated number of words. Returns 0 if input is not a string.
    """
    if not isinstance(text, str):
        return 0
    return len(text.split())

def sanitize_filename(name: Any) -> str:
    """
    Cleans a string to make it a valid and readable filename.

    This function performs several operations:
    - Converts the input to a string if it's not already.
    - Removes characters that are invalid in Windows, Linux, and macOS filenames.
    - Collapses consecutive whitespace characters into a single underscore.
    - Removes leading/trailing dots, underscores, and spaces.
    - Prevents multiple consecutive dots.
    - Truncates the filename to a safe length (100 chars).
    - Provides a default name if the sanitization results in an empty string.

    Args:
        name: The input value to be converted into a safe filename.

    Returns:
        A sanitized, safe-to-use string for a filename.
    """
    if not isinstance(name, str):
        name = str(name)

    # Remove characters invalid for Windows/Linux/MacOS filenames.
    sanitized: str = re.sub(r'[\\/*?:"<>|\']', "", name)

    # Replace multiple spaces, tabs, or newlines with a single underscore.
    sanitized = re.sub(r'\s+', '_', sanitized)

    # Replace two or more consecutive dots with a single dot.
    sanitized = re.sub(r'\.{2,}', '.', sanitized)

    # Remove leading/trailing dots, underscores, or spaces.
    sanitized = sanitized.strip('._ ')

    # If the cleaning process resulted in an empty string, provide a default.
    if not sanitized:
        return "invalid_title"

    # Limit the total length of the filename to a reasonable maximum.
    max_len = 100
    if len(sanitized) > max_len:
        # Truncate and ensure it doesn't end with a problematic character.
        sanitized = sanitized[:max_len].rstrip('._-')

    return sanitized


def replace_guillemets_text(text: str) -> str:
    """
    Replaces guillemet characters (« ») with standard double quotes (").
    
    This is a non‑destructive replacement that preserves the semantic meaning
    of quotation marks while ensuring compatibility with TTS engines that may
    not handle guillemets correctly.
    
    Args:
        text: Input string possibly containing guillemets.
        
    Returns:
        String with « replaced by " and » replaced by ".
    """
    if not isinstance(text, str):
        return text
    return text.replace('«', '"').replace('»', '"')


def parse_dialogue_script(script_text: str) -> list[dict]:
    """
    Parses text with [Actor] tags and splits it into a list of dictionaries.
    E.g.: [{'actor': 'Narrator', 'text': '...'}, {'actor': 'Protagonist', 'text': '...'}]
    """
    if not script_text:
        return []
    
    # Regex to find [Actor] tags
    pattern = r"\[([a-zA-Z0-9_\s]+)\]"
    
    parts = re.split(pattern, script_text)
    
    dialogue = []
    # The first element is text before the first tag, which we skip
    # Then we process pairs: actor, text
    for i in range(1, len(parts), 2):
        actor = parts[i].strip()
        text = parts[i+1].strip()
        if actor and text:
            dialogue.append({"actor": actor, "text": text})
    
    return dialogue
