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

import os
import re
import warnings
import ebooklib
from ebooklib import epub
from bs4 import BeautifulSoup, XMLParsedAsHTMLWarning
from . import utils
import traceback
from typing import Optional, Dict

FALLBACK_CHAPTER_PATTERN = re.compile(r"^\s*(\d+)\s*$", re.MULTILINE)

# --- EPUB Text Extraction Functions ---

def extract_text_from_item(item) -> str:
    """Extracts clean text content from an EPUB item (HTML/XHTML document)."""
    try:
        content_bytes = item.get_content()
        parser_to_use = 'html.parser'
        lxml_parsing = False
        try:
            import lxml
            parser_to_use = 'lxml'
            lxml_parsing = True
        except ImportError:
            pass

        soup = None
        if lxml_parsing:
            with warnings.catch_warnings():
                warnings.filterwarnings("ignore", category=XMLParsedAsHTMLWarning)
                soup = BeautifulSoup(content_bytes, features='lxml')
        else:
            soup = BeautifulSoup(content_bytes, features='html.parser')

        tags_to_remove = [
            'script', 'style', 'nav', 'header', 'footer', 'aside', 'figure',
            'img', 'a', 'sup', 'sub', 'hr', 'br', 'map', 'area', 'noscript',
            'iframe', 'object', 'embed', 'video', 'audio', 'source', 'track'
        ]
        for tag_name in tags_to_remove:
            for tag in soup.find_all(tag_name):
                 if tag_name in ['br', 'hr']:
                     tag.replace_with('\n')
                 else:
                     tag.decompose()

        text = soup.get_text(separator='\n', strip=True)
        text = re.sub(r'[ \t]+', ' ', text)
        text = re.sub(r'\n\s*\n', '\n', text)
        return text.strip()
    except Exception as e:
        parser_desc = "lxml (HTML mode)" if lxml_parsing else "html.parser"
        print(f"    WARNING: Could not extract text from item '{item.get_name()}' using {parser_desc}: {e}")
        if lxml_parsing:
             try:
                 print("      Retrying with html.parser...")
                 soup = BeautifulSoup(content_bytes, features='html.parser')
                 for tag_name in tags_to_remove:
                     for tag in soup.find_all(tag_name):
                          if tag_name in ['br', 'hr']: tag.replace_with('\n')
                          else: tag.decompose()
                 text = soup.get_text(separator='\n', strip=True)
                 text = re.sub(r'[ \t]+', ' ', text)
                 text = re.sub(r'\n\s*\n', '\n', text)
                 return text.strip()
             except Exception as e2:
                 print(f"      ERROR: Fallback extraction with html.parser also failed: {e2}")
        return ""

def extract_chapters_from_epub(epub_path: str) -> Optional[Dict[str, str]]:
    """
    Extracts chapters from an EPUB file, trying ToC first, then fallback.
    Returns a dictionary of chapters.
    """
    if not os.path.isfile(epub_path):
        print(f"  ERROR: File does not exist: {epub_path}")
        return None

    try:
        book = epub.read_epub(epub_path, options={'ignore_ncx': True})
    except Exception as e:
        print(f"  ERROR: Failed to read EPUB: {e}")
        traceback.print_exc()
        return None

    chapters = {}
    processed_items = set()
    chapter_count = 0

    if book.toc:
        flat_toc = []
        def flatten_toc_items(toc_items):
            for item in toc_items:
                if isinstance(item, epub.Link):
                    flat_toc.append(item)
                elif isinstance(item, (list, tuple)) and len(item) > 0:
                    if isinstance(item[0], epub.Link):
                        flat_toc.append(item[0])
                    if len(item) > 1 and isinstance(item[1], (list, tuple)):
                        flatten_toc_items(item[1])
        
        flatten_toc_items(book.toc)

        content_map = {item.get_name(): item for item in book.get_items()}
        
        for i, link in enumerate(flat_toc):
            href_base = link.href.split('#')[0]
            item = content_map.get(href_base)
            
            if item and item.get_type() == ebooklib.ITEM_DOCUMENT and item not in processed_items:
                chapter_text = extract_text_from_item(item)
                if chapter_text and len(chapter_text.strip()) > 50:
                    raw_title = link.title or f"Chapter_{i+1}"
                    clean_title = utils.sanitize_filename(raw_title)
                    chapter_key = f"Chapter_{chapter_count + 1:02d}_{clean_title}"
                    if len(chapter_key) > 80: chapter_key = chapter_key[:77] + "..."
                    chapters[chapter_key] = chapter_text.strip()
                    processed_items.add(item)
                    chapter_count += 1

    if chapter_count > 0:
        print(f"  Successfully extracted {chapter_count} chapters via ToC.")
        return chapters

    # --- Fallback Logic ---
    print("  WARNING: ToC not found or did not yield chapters. Using fallback sequential extraction.")
    full_text = ""
    # Use spine order if available, otherwise process all documents.
    items_in_order = book.spine if book.spine else list(book.get_items_of_type(ebooklib.ITEM_DOCUMENT))
    for item_id in items_in_order:
        item = book.get_item_with_id(item_id[0]) if isinstance(item_id, tuple) else item_id
        if item and item.get_type() == ebooklib.ITEM_DOCUMENT and item not in processed_items:
            text = extract_text_from_item(item)
            if text and len(text.strip()) > 50:
                full_text += text.strip() + "\n\n"
    
    if full_text:
        print("  Fallback: Treating entire book content as a single chapter.")
        return {"Chapter_01_Full_Book": full_text.strip()}

    print("  ERROR: Failed to extract any meaningful content from the EPUB.")
    return None

# --- Text Chunking Functions ---

def split_into_sentences(text: str) -> list[str]:
    if not text: return []
    text = re.sub(r'\s*\n\s*', ' ', text).strip()
    # Positive lookbehind for sentence-ending punctuation, avoiding ellipses.
    sentences = re.split(r'(?<=[.!?])(?<!\.\.)\s+', text)
    return [s.strip() for s in sentences if s and s.strip()]

def split_long_sentence(sentence: str, max_length: int) -> list[str]:
    """
    Splits a long sentence into subparts trying to preserve punctuation boundaries.
    """
    # First, try to split by commas, semicolons, conjunctions
    # Define split patterns in order of preference
    patterns = [r'[,;]', r'\s+[e|o|ma|però|quindi|dunque|allora|cioè]\s+', r'\s+']
    for pattern in patterns:
        parts = re.split(f'({pattern})', sentence)
        # Recombine with delimiter
        combined = []
        current = ""
        for i in range(0, len(parts), 2):
            part = parts[i]
            delimiter = parts[i+1] if i+1 < len(parts) else ""
            if len(current) + len(part) + len(delimiter) <= max_length:
                current += part + delimiter
            else:
                if current:
                    combined.append(current.strip())
                current = part + delimiter
        if current:
            combined.append(current.strip())
        # Check if all parts are within limit
        if all(len(p) <= max_length for p in combined):
            return combined
    # Fallback: split by words
    words = sentence.split()
    parts = []
    current = ""
    for word in words:
        if len(current) + len(word) + 1 <= max_length:
            current += " " + word if current else word
        else:
            if current:
                parts.append(current)
            current = word
    if current:
        parts.append(current)
    return parts

def chunk_chapter_text(chapter_text, use_char_limit_chunking, max_chars_per_chunk, min_words_approx, max_words_approx, replace_guillemets=False, **kwargs):
    """
    Splits chapter text into chunks. Includes robust splitting for sentences
    that exceed the character limit, trying to preserve natural boundaries.
    """
    # Apply guillemet replacement if requested
    if replace_guillemets:
        from . import utils
        chapter_text = utils.replace_guillemets_text(chapter_text)
    
    sentences = split_into_sentences(chapter_text)
    if not sentences:
        print("    WARNING: No sentences found after splitting. Cannot chunk chapter.")
        return []

    chunks = []
    
    if use_char_limit_chunking:
        print(f"    Chunking strategy: Character limit (max: {max_chars_per_chunk} chars)")
        current_chunk = ""
        for sentence in sentences:
            if not sentence: continue

            if len(sentence) > max_chars_per_chunk:
                print(f"    INFO: Sentence exceeds max_chars limit ({len(sentence)} > {max_chars_per_chunk}). Splitting it: '{sentence[:80]}...'")
                if current_chunk:
                    chunks.append(current_chunk)
                    current_chunk = ""
                
                sub_parts = split_long_sentence(sentence, max_chars_per_chunk)
                chunks.extend(p for p in sub_parts if p)
                continue

            if current_chunk and len(current_chunk) + len(sentence) + 1 > max_chars_per_chunk:
                chunks.append(current_chunk)
                current_chunk = sentence
            else:
                current_chunk = f"{current_chunk} {sentence}" if current_chunk else sentence

        if current_chunk:
            chunks.append(current_chunk)
            
    else: # Word Count Chunking Logic
        print(f"    Chunking strategy: Word count approx (target: {min_words_approx}-{max_words_approx} words)")
        current_chunk_sentences = []
        current_word_count = 0
        for sentence in sentences:
            sentence_word_count = utils.count_words_proxy(sentence)
            if sentence_word_count == 0: continue

            # If adding the sentence would push the chunk over the max limit,
            # and the chunk is already reasonably large, start a new chunk.
            if current_chunk_sentences and (current_word_count + sentence_word_count > max_words_approx) and (current_word_count >= min_words_approx):
                chunks.append(" ".join(current_chunk_sentences).strip())
                current_chunk_sentences = [sentence]
                current_word_count = sentence_word_count
            else:
                current_chunk_sentences.append(sentence)
                current_word_count += sentence_word_count

        if current_chunk_sentences:
            chunks.append(" ".join(current_chunk_sentences).strip())

    final_chunks = [c.strip() for c in chunks if c.strip()]
    print(f"    Divided into {len(final_chunks)} final chunks.")
    return final_chunks
