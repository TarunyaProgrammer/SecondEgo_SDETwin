"""Gesture-based control layer for SecondEgo.

Detects hand gestures via OpenCV + MediaPipe and maps them to
engine actions, publishing events over a local Unix/TCP socket
that the desktop gateway subscribes to.
"""
