"""
LLM Provider Abstraction for Ganesha 3.0

Clean, async-first provider abstraction supporting:
- LM Studio (local)
- Ollama (local)
- Anthropic Claude (cloud)
- OpenAI (cloud)

Prioritizes local providers for privacy and cost efficiency.
"""

import asyncio
import json
import os
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

# Use aiohttp for async HTTP, fall back to requests
try:
    import aiohttp
    HAS_AIOHTTP = True
except ImportError:
    HAS_AIOHTTP = False
    import requests


@dataclass
class LLMResponse:
    """Standardized LLM response."""
    content: str
    model: str
    provider: str
    tokens_used: Optional[int] = None
    error: Optional[str] = None


class LLMProvider(ABC):
    """Base class for LLM providers."""

    name: str = "base"

    @abstractmethod
    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120,
    ) -> LLMResponse:
        """Generate response from LLM."""
        pass

    @abstractmethod
    def is_available(self) -> bool:
        """Check if provider is available."""
        pass

    @abstractmethod
    def list_models(self) -> List[str]:
        """List available models."""
        pass


class LMStudioProvider(LLMProvider):
    """LM Studio provider - local OpenAI-compatible API."""

    name = "lmstudio"

    def __init__(self, url: str = "http://localhost:1234", model: Optional[str] = None):
        self.url = url.rstrip("/")
        self.model = model
        self._cached_models: Optional[List[str]] = None

    def is_available(self) -> bool:
        try:
            if HAS_AIOHTTP:
                # Sync check for availability
                import requests
            response = requests.get(f"{self.url}/v1/models", timeout=5)
            return response.status_code == 200
        except Exception:
            return False

    def list_models(self) -> List[str]:
        if self._cached_models:
            return self._cached_models
        try:
            import requests
            response = requests.get(f"{self.url}/v1/models", timeout=5)
            if response.ok:
                data = response.json()
                self._cached_models = [m["id"] for m in data.get("data", [])]
                return self._cached_models
        except Exception:
            pass
        return []

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120,
    ) -> LLMResponse:
        model = self.model or (self.list_models() or ["default"])[0]

        payload = {
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": False,
        }

        try:
            if HAS_AIOHTTP:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        f"{self.url}/v1/chat/completions",
                        json=payload,
                        timeout=aiohttp.ClientTimeout(total=timeout),
                    ) as response:
                        if not response.ok:
                            return LLMResponse(
                                content="",
                                model=model,
                                provider=self.name,
                                error=f"HTTP {response.status}",
                            )
                        data = await response.json()
            else:
                # Sync fallback
                import requests
                response = requests.post(
                    f"{self.url}/v1/chat/completions",
                    json=payload,
                    timeout=timeout,
                )
                if not response.ok:
                    return LLMResponse(
                        content="",
                        model=model,
                        provider=self.name,
                        error=f"HTTP {response.status_code}",
                    )
                data = response.json()

            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            tokens = data.get("usage", {}).get("total_tokens")

            return LLMResponse(
                content=content,
                model=model,
                provider=self.name,
                tokens_used=tokens,
            )

        except asyncio.TimeoutError:
            return LLMResponse(content="", model=model, provider=self.name, error="Timeout")
        except Exception as e:
            return LLMResponse(content="", model=model, provider=self.name, error=str(e))


class OllamaProvider(LLMProvider):
    """Ollama provider - local LLM."""

    name = "ollama"

    def __init__(self, url: str = "http://localhost:11434", model: str = "llama3"):
        self.url = url.rstrip("/")
        self.model = model

    def is_available(self) -> bool:
        try:
            import requests
            response = requests.get(f"{self.url}/api/tags", timeout=5)
            return response.status_code == 200
        except Exception:
            return False

    def list_models(self) -> List[str]:
        try:
            import requests
            response = requests.get(f"{self.url}/api/tags", timeout=5)
            if response.ok:
                return [m["name"] for m in response.json().get("models", [])]
        except Exception:
            pass
        return []

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120,
    ) -> LLMResponse:
        payload = {
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            "options": {"temperature": temperature, "num_predict": max_tokens},
            "stream": False,
        }

        try:
            if HAS_AIOHTTP:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        f"{self.url}/api/chat",
                        json=payload,
                        timeout=aiohttp.ClientTimeout(total=timeout),
                    ) as response:
                        if not response.ok:
                            return LLMResponse(
                                content="", model=self.model, provider=self.name,
                                error=f"HTTP {response.status}"
                            )
                        data = await response.json()
            else:
                import requests
                response = requests.post(f"{self.url}/api/chat", json=payload, timeout=timeout)
                data = response.json()

            return LLMResponse(
                content=data.get("message", {}).get("content", ""),
                model=self.model,
                provider=self.name,
            )
        except Exception as e:
            return LLMResponse(content="", model=self.model, provider=self.name, error=str(e))


class AnthropicProvider(LLMProvider):
    """Anthropic Claude provider."""

    name = "anthropic"

    def __init__(self, api_key: Optional[str] = None, model: str = "claude-sonnet-4-20250514"):
        self.api_key = api_key or os.getenv("ANTHROPIC_API_KEY")
        self.model = model
        self.url = "https://api.anthropic.com/v1/messages"

    def is_available(self) -> bool:
        return bool(self.api_key)

    def list_models(self) -> List[str]:
        return ["claude-opus-4-20250514", "claude-sonnet-4-20250514", "claude-3-5-haiku-20241022"]

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120,
    ) -> LLMResponse:
        if not self.api_key:
            return LLMResponse(content="", model=self.model, provider=self.name, error="No API key")

        headers = {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "anthropic-version": "2023-06-01",
        }
        payload = {
            "model": self.model,
            "max_tokens": max_tokens,
            "system": system_prompt,
            "messages": [{"role": "user", "content": user_prompt}],
            "temperature": temperature,
        }

        try:
            if HAS_AIOHTTP:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        self.url, json=payload, headers=headers,
                        timeout=aiohttp.ClientTimeout(total=timeout)
                    ) as response:
                        data = await response.json()
            else:
                import requests
                response = requests.post(self.url, json=payload, headers=headers, timeout=timeout)
                data = response.json()

            content = data.get("content", [{}])[0].get("text", "")
            tokens = data.get("usage", {}).get("input_tokens", 0) + data.get("usage", {}).get("output_tokens", 0)

            return LLMResponse(content=content, model=self.model, provider=self.name, tokens_used=tokens)
        except Exception as e:
            return LLMResponse(content="", model=self.model, provider=self.name, error=str(e))


class OpenAIProvider(LLMProvider):
    """OpenAI provider (legacy support)."""

    name = "openai"

    def __init__(self, api_key: Optional[str] = None, model: str = "gpt-4o"):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY")
        self.model = model
        self.url = "https://api.openai.com/v1/chat/completions"

    def is_available(self) -> bool:
        return bool(self.api_key)

    def list_models(self) -> List[str]:
        return ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo"]

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120,
    ) -> LLMResponse:
        if not self.api_key:
            return LLMResponse(content="", model=self.model, provider=self.name, error="No API key")

        headers = {"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"}
        payload = {
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            "temperature": temperature,
            "max_tokens": max_tokens,
        }

        try:
            if HAS_AIOHTTP:
                async with aiohttp.ClientSession() as session:
                    async with session.post(
                        self.url, json=payload, headers=headers,
                        timeout=aiohttp.ClientTimeout(total=timeout)
                    ) as response:
                        data = await response.json()
            else:
                import requests
                response = requests.post(self.url, json=payload, headers=headers, timeout=timeout)
                data = response.json()

            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            tokens = data.get("usage", {}).get("total_tokens")

            return LLMResponse(content=content, model=self.model, provider=self.name, tokens_used=tokens)
        except Exception as e:
            return LLMResponse(content="", model=self.model, provider=self.name, error=str(e))


class ProviderChain:
    """Chain of providers with fallback."""

    def __init__(self, providers: List[LLMProvider]):
        self.providers = providers

    def get_available_providers(self) -> List[LLMProvider]:
        return [p for p in self.providers if p.is_available()]

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        **kwargs,
    ) -> LLMResponse:
        errors = []
        for provider in self.providers:
            if not provider.is_available():
                errors.append(f"{provider.name}: unavailable")
                continue
            response = await provider.generate(system_prompt, user_prompt, **kwargs)
            if not response.error:
                return response
            errors.append(f"{provider.name}: {response.error}")

        return LLMResponse(content="", model="", provider="chain", error="; ".join(errors))


class AsyncProviderWrapper:
    """Wrapper to make ProviderChain usable as async LLMProvider."""

    def __init__(self, chain: ProviderChain):
        self.chain = chain

    async def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
    ) -> str:
        response = await self.chain.generate(
            system_prompt=system_prompt,
            user_prompt=user_prompt,
            temperature=temperature,
            max_tokens=max_tokens,
        )
        if response.error:
            raise RuntimeError(response.error)
        return response.content

    def is_available(self) -> bool:
        return bool(self.chain.get_available_providers())


def create_provider_chain() -> ProviderChain:
    """
    Create default provider chain.

    Order (local-first):
    1. LM Studio BEAST
    2. LM Studio BEDROOM
    3. Anthropic Claude
    4. OpenAI
    """
    return ProviderChain([
        LMStudioProvider(url="http://192.168.245.155:1234"),  # BEAST
        LMStudioProvider(url="http://192.168.27.182:1234"),   # BEDROOM
        AnthropicProvider(),
        OpenAIProvider(),
    ])


# Alias for backwards compatibility
create_default_chain = create_provider_chain
