"""
LLM Provider Abstraction Layer for Ganesha

Supports:
- LM Studio (OpenAI-compatible local)
- Ollama (local)
- Anthropic Claude (cloud)
- OpenAI (cloud, legacy support)

This allows Ganesha to work with any LLM, prioritizing local models.
"""

import os
import json
import requests
from abc import ABC, abstractmethod
from typing import Optional, Dict, Any, List
from dataclasses import dataclass


@dataclass
class LLMResponse:
    """Standardized response from any LLM provider."""
    content: str
    model: str
    provider: str
    tokens_used: Optional[int] = None
    error: Optional[str] = None


class LLMProvider(ABC):
    """Abstract base class for LLM providers."""

    name: str = "base"

    @abstractmethod
    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120
    ) -> LLMResponse:
        """Generate a response from the LLM."""
        pass

    @abstractmethod
    def is_available(self) -> bool:
        """Check if the provider is available and configured."""
        pass

    @abstractmethod
    def list_models(self) -> List[str]:
        """List available models for this provider."""
        pass


class LMStudioProvider(LLMProvider):
    """
    LM Studio provider - local LLM server with OpenAI-compatible API.
    """

    name = "lmstudio"

    def __init__(self, url: str = "http://localhost:1234", model: Optional[str] = None):
        self.url = url.rstrip("/")
        self.model = model
        self._cached_models: Optional[List[str]] = None

    def is_available(self) -> bool:
        try:
            response = requests.get(
                f"{self.url}/v1/models",
                timeout=5
            )
            return response.status_code == 200
        except:
            return False

    def list_models(self) -> List[str]:
        if self._cached_models:
            return self._cached_models
        try:
            response = requests.get(f"{self.url}/v1/models", timeout=5)
            if response.ok:
                data = response.json()
                self._cached_models = [m["id"] for m in data.get("data", [])]
                return self._cached_models
        except:
            pass
        return []

    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120
    ) -> LLMResponse:
        # Get model if not specified
        model = self.model
        if not model:
            models = self.list_models()
            if models:
                model = models[0]
            else:
                return LLMResponse(
                    content="",
                    model="",
                    provider=self.name,
                    error="No models available"
                )

        try:
            response = requests.post(
                f"{self.url}/v1/chat/completions",
                headers={"Content-Type": "application/json"},
                json={
                    "model": model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": temperature,
                    "max_tokens": max_tokens,
                    "stream": False
                },
                timeout=timeout
            )

            if not response.ok:
                return LLMResponse(
                    content="",
                    model=model,
                    provider=self.name,
                    error=f"HTTP {response.status_code}: {response.text[:200]}"
                )

            data = response.json()
            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            tokens = data.get("usage", {}).get("total_tokens")

            return LLMResponse(
                content=content,
                model=model,
                provider=self.name,
                tokens_used=tokens
            )

        except requests.Timeout:
            return LLMResponse(
                content="",
                model=model,
                provider=self.name,
                error="Request timed out"
            )
        except Exception as e:
            return LLMResponse(
                content="",
                model=model,
                provider=self.name,
                error=str(e)
            )


class OllamaProvider(LLMProvider):
    """
    Ollama provider - local LLM server.
    """

    name = "ollama"

    def __init__(self, url: str = "http://localhost:11434", model: str = "llama3"):
        self.url = url.rstrip("/")
        self.model = model

    def is_available(self) -> bool:
        try:
            response = requests.get(f"{self.url}/api/tags", timeout=5)
            return response.status_code == 200
        except:
            return False

    def list_models(self) -> List[str]:
        try:
            response = requests.get(f"{self.url}/api/tags", timeout=5)
            if response.ok:
                data = response.json()
                return [m["name"] for m in data.get("models", [])]
        except:
            pass
        return []

    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120
    ) -> LLMResponse:
        try:
            response = requests.post(
                f"{self.url}/api/chat",
                json={
                    "model": self.model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "options": {
                        "temperature": temperature,
                        "num_predict": max_tokens
                    },
                    "stream": False
                },
                timeout=timeout
            )

            if not response.ok:
                return LLMResponse(
                    content="",
                    model=self.model,
                    provider=self.name,
                    error=f"HTTP {response.status_code}"
                )

            data = response.json()
            content = data.get("message", {}).get("content", "")

            return LLMResponse(
                content=content,
                model=self.model,
                provider=self.name
            )

        except Exception as e:
            return LLMResponse(
                content="",
                model=self.model,
                provider=self.name,
                error=str(e)
            )


class AnthropicProvider(LLMProvider):
    """
    Anthropic Claude provider - cloud LLM.
    """

    name = "anthropic"

    def __init__(self, api_key: Optional[str] = None, model: str = "claude-sonnet-4-20250514"):
        self.api_key = api_key or os.getenv("ANTHROPIC_API_KEY")
        self.model = model
        self.url = "https://api.anthropic.com/v1/messages"

    def is_available(self) -> bool:
        return bool(self.api_key)

    def list_models(self) -> List[str]:
        # Anthropic doesn't have a models endpoint, return known models
        return [
            "claude-opus-4-20250514",
            "claude-sonnet-4-20250514",
            "claude-3-5-sonnet-20241022",
            "claude-3-5-haiku-20241022"
        ]

    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120
    ) -> LLMResponse:
        if not self.api_key:
            return LLMResponse(
                content="",
                model=self.model,
                provider=self.name,
                error="No API key configured"
            )

        try:
            response = requests.post(
                self.url,
                headers={
                    "Content-Type": "application/json",
                    "x-api-key": self.api_key,
                    "anthropic-version": "2023-06-01"
                },
                json={
                    "model": self.model,
                    "max_tokens": max_tokens,
                    "system": system_prompt,
                    "messages": [
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": temperature
                },
                timeout=timeout
            )

            if not response.ok:
                return LLMResponse(
                    content="",
                    model=self.model,
                    provider=self.name,
                    error=f"HTTP {response.status_code}: {response.text[:200]}"
                )

            data = response.json()
            content = data.get("content", [{}])[0].get("text", "")
            tokens = data.get("usage", {}).get("input_tokens", 0) + data.get("usage", {}).get("output_tokens", 0)

            return LLMResponse(
                content=content,
                model=self.model,
                provider=self.name,
                tokens_used=tokens
            )

        except Exception as e:
            return LLMResponse(
                content="",
                model=self.model,
                provider=self.name,
                error=str(e)
            )


class OpenAIProvider(LLMProvider):
    """
    OpenAI provider - cloud LLM (legacy support).
    Uses new SDK style but maintains compatibility.
    """

    name = "openai"

    def __init__(self, api_key: Optional[str] = None, model: str = "gpt-4o"):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY")
        self.model = model
        self.url = "https://api.openai.com/v1/chat/completions"

    def is_available(self) -> bool:
        return bool(self.api_key)

    def list_models(self) -> List[str]:
        return ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-3.5-turbo"]

    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        temperature: float = 0.3,
        max_tokens: int = 2000,
        timeout: int = 120
    ) -> LLMResponse:
        if not self.api_key:
            return LLMResponse(
                content="",
                model=self.model,
                provider=self.name,
                error="No API key configured"
            )

        try:
            response = requests.post(
                self.url,
                headers={
                    "Content-Type": "application/json",
                    "Authorization": f"Bearer {self.api_key}"
                },
                json={
                    "model": self.model,
                    "messages": [
                        {"role": "system", "content": system_prompt},
                        {"role": "user", "content": user_prompt}
                    ],
                    "temperature": temperature,
                    "max_tokens": max_tokens
                },
                timeout=timeout
            )

            if not response.ok:
                return LLMResponse(
                    content="",
                    model=self.model,
                    provider=self.name,
                    error=f"HTTP {response.status_code}"
                )

            data = response.json()
            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            tokens = data.get("usage", {}).get("total_tokens")

            return LLMResponse(
                content=content,
                model=self.model,
                provider=self.name,
                tokens_used=tokens
            )

        except Exception as e:
            return LLMResponse(
                content="",
                model=self.model,
                provider=self.name,
                error=str(e)
            )


class ProviderChain:
    """
    Chain of providers with fallback support.
    Tries providers in order until one succeeds.
    """

    def __init__(self, providers: List[LLMProvider]):
        self.providers = providers

    def get_available_providers(self) -> List[LLMProvider]:
        """Get list of currently available providers."""
        return [p for p in self.providers if p.is_available()]

    def generate(
        self,
        system_prompt: str,
        user_prompt: str,
        **kwargs
    ) -> LLMResponse:
        """Try each provider in order until one succeeds."""
        errors = []

        for provider in self.providers:
            if not provider.is_available():
                errors.append(f"{provider.name}: not available")
                continue

            response = provider.generate(system_prompt, user_prompt, **kwargs)

            if not response.error:
                return response

            errors.append(f"{provider.name}: {response.error}")

        # All providers failed
        return LLMResponse(
            content="",
            model="",
            provider="chain",
            error=f"All providers failed: {'; '.join(errors)}"
        )


def create_default_chain() -> ProviderChain:
    """
    Create a default provider chain prioritizing local LLMs.

    Order:
    1. LM Studio (BEAST - powerful local)
    2. LM Studio (BEDROOM - backup local)
    3. Anthropic Claude (cloud fallback)
    4. OpenAI (legacy cloud fallback)
    """
    providers = [
        LMStudioProvider(url="http://192.168.245.155:1234"),  # BEAST
        LMStudioProvider(url="http://192.168.27.182:1234"),   # BEDROOM
        AnthropicProvider(),
        OpenAIProvider()
    ]

    return ProviderChain(providers)


def create_provider_from_config(config: Dict[str, Any]) -> LLMProvider:
    """Create a provider from configuration dictionary."""
    provider_type = config.get("type", "lmstudio")

    if provider_type == "lmstudio":
        return LMStudioProvider(
            url=config.get("url", "http://localhost:1234"),
            model=config.get("model")
        )
    elif provider_type == "ollama":
        return OllamaProvider(
            url=config.get("url", "http://localhost:11434"),
            model=config.get("model", "llama3")
        )
    elif provider_type == "anthropic":
        return AnthropicProvider(
            api_key=config.get("api_key"),
            model=config.get("model", "claude-sonnet-4-20250514")
        )
    elif provider_type == "openai":
        return OpenAIProvider(
            api_key=config.get("api_key"),
            model=config.get("model", "gpt-4o")
        )
    else:
        raise ValueError(f"Unknown provider type: {provider_type}")


# Test function
def test_providers():
    """Test all available providers."""
    print("Testing LLM Providers\n" + "=" * 50)

    chain = create_default_chain()
    available = chain.get_available_providers()

    print(f"\nAvailable providers: {len(available)}/{len(chain.providers)}")
    for p in available:
        print(f"  - {p.name}: {p.url if hasattr(p, 'url') else 'cloud'}")
        models = p.list_models()
        if models:
            print(f"    Models: {', '.join(models[:3])}{'...' if len(models) > 3 else ''}")

    print("\nTesting generation...")
    response = chain.generate(
        system_prompt="You are a helpful assistant. Be concise.",
        user_prompt="What is 2 + 2? Answer in one word."
    )

    if response.error:
        print(f"Error: {response.error}")
    else:
        print(f"Provider: {response.provider}")
        print(f"Model: {response.model}")
        print(f"Response: {response.content}")
        if response.tokens_used:
            print(f"Tokens: {response.tokens_used}")


if __name__ == "__main__":
    test_providers()
