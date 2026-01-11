"""
Ganesha - The Remover of Obstacles

AI-powered system control and code generation.
Local-first, safe by default.

Install:
    pip install -e .

    # With all features:
    pip install -e ".[all]"
"""

from setuptools import setup, find_packages
from pathlib import Path

readme = Path(__file__).parent / "README.md"
long_description = readme.read_text() if readme.exists() else ""

setup(
    name="ganesha-ai",
    version="3.0.0",
    author="G-Tech SD",
    author_email="dev@gtechsd.com",
    description="The Remover of Obstacles - AI-Powered System Control",
    long_description=long_description,
    long_description_content_type="text/markdown",
    url="https://github.com/G-TechSD/ganesha-ai",
    packages=find_packages(),
    python_requires=">=3.10",
    install_requires=[
        "requests>=2.28.0",
        "colorama>=0.4.6",
    ],
    extras_require={
        "cli": [
            "colorama>=0.4.6",
        ],
        "api": [
            "fastapi>=0.100.0",
            "uvicorn>=0.23.0",
            "pydantic>=2.0.0",
        ],
        "mcp": [
            "mcp>=0.1.0",
        ],
        "async": [
            "aiohttp>=3.8.0",
        ],
        "all": [
            "colorama>=0.4.6",
            "fastapi>=0.100.0",
            "uvicorn>=0.23.0",
            "pydantic>=2.0.0",
            "aiohttp>=3.8.0",
            "psutil>=5.9.0",
        ],
    },
    entry_points={
        "console_scripts": [
            "ganesha=ganesha.cli.main:main",
            "ganesha-api=ganesha.api.server:main",
            "ganesha-mcp=ganesha.mcp.server:main",
            "ganesha-daemon=ganesha.daemon.privileged:main",
            "ganesha-config=ganesha.daemon.config_cli:main",
        ],
    },
    classifiers=[
        "Development Status :: 4 - Beta",
        "Environment :: Console",
        "Intended Audience :: Developers",
        "Intended Audience :: System Administrators",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Python :: 3.12",
        "Topic :: System :: Systems Administration",
        "Topic :: Software Development :: Code Generators",
    ],
    keywords="ai llm cli system-administration code-generation local-llm",
)
