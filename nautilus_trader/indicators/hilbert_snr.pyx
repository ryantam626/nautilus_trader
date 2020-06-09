# -------------------------------------------------------------------------------------------------
#  Copyright (C) 2015-2020 Nautech Systems Pty Ltd. All rights reserved.
#  The use of this source code is governed by the license as found in the LICENSE file.
#  https://nautechsystems.io
# -------------------------------------------------------------------------------------------------

import cython
import numpy as np
from collections import deque

from nautilus_trader.indicators.base.indicator cimport Indicator
from nautilus_trader.core.correctness cimport Condition


cdef class HilbertSignalNoiseRatio(Indicator):
    """
    An indicator which calculates the amplitude of a signal.
    """

    def __init__(self,
                 int period=7,
                 double range_floor=0.00001,
                 double amplitude_floor=0.001,
                 bint check_inputs=False):
        """
        Initializes a new instance of the HilbertSignalNoiseRatio class.

        :param period: The rolling window period for the indicator (> 0).
        :param range_floor: The floor value for range calculations.
        :param amplitude_floor: The floor value for amplitude calculations (0.001 from paper).
        :param check_inputs: The flag indicating whether the input values should be checked.
        """
        Condition.positive_int(period, 'period')
        Condition.not_negative(range_floor, 'range_floor')
        Condition.not_negative(amplitude_floor, 'amplitude_floor')

        super().__init__(params=[period], check_inputs=check_inputs)
        self.period = period
        self._i_mult = 0.635
        self._q_mult = 0.338
        self._range_floor = range_floor
        self._amplitude_floor = amplitude_floor
        self._inputs = deque(maxlen=self.period)
        self._detrended_prices = deque(maxlen=self.period)
        self._in_phase = deque([0.0] * self.period, maxlen=self.period)
        self._quadrature = deque([0.0] * self.period, maxlen=self.period)
        self._previous_range = 0.0
        self._previous_amplitude = 0.0
        self._previous_value = 0.0
        self._range = 0.0
        self._amplitude = 0.0
        self.value = 0.0  # The last amplitude value (dB)

    @cython.binding(True)
    cpdef void update(self,double high, double low):
        """
        Update the indicator with the given price values.

        :param high: The high price (> 0).
        :param low: The low price (> 0).
        """
        if self.check_inputs:
            Condition.positive(high, 'high')
            Condition.positive(low, 'low')
            Condition.true(high >= low, 'high >= low')

        self._inputs.append((high + low) / 2.0)

        # Initialization logic
        if not self.initialized:
            # Do not initialize __has_inputs here
            if len(self._inputs) >= self.period:
                self._set_initialized()
            else:
                return

        # Update de-trended prices
        self._detrended_prices.append(self._inputs[-1] - self._inputs[0])

        cdef double last_range = high - low
        # Must have a trading range to calculate
        if last_range == 0.0:
            return

        # If initial input then initialize ranges
        if self._previous_range == 0.0:
            self._previous_range = last_range

        # Compute 'noise' as the average (smoothed) range
        self._range = max((0.2 * last_range) + (0.8 * self._previous_range), self._range_floor)
        self._previous_range = self._range

        # If insufficient de-trended prices to index feedback then return
        if len(self._detrended_prices) < 5:
            return

        self._calc_hilbert_transform()

        cdef double amplitude = self._calc_amplitude()
        # If initial input then initialize amplitude
        if self._previous_amplitude == 0:
            self._previous_amplitude = amplitude

        # Calculate smoothed signal amplitude
        self._amplitude = max((0.2 * amplitude) + (0.8 * self._previous_amplitude), self._amplitude_floor)
        self._previous_amplitude = self._amplitude

        if self._range == 0.0:
            return  # Cannot calculate (guarding against divide by zero)

        # If initial input then initialize value
        if not self.has_inputs:
            self.value = self._calc_signal_noise_ratio()
            self._previous_value = self.value
            self._set_has_inputs()

        # Compute smoothed SNR in Decibels
        self.value = (0.25 * self._calc_signal_noise_ratio()) + (0.75 * self._previous_value)
        self._previous_value = self.value

    cdef void _calc_hilbert_transform(self):
        """
        Calculate the Hilbert Transform and update in-phase and quadrature values.
        """
        # Calculate feedback
        cdef double feedback1 = self._detrended_prices[-1]  # V1 (last)
        cdef double feedback2 = self._detrended_prices[-3]  # V2 (2 elements back from the last)
        cdef double feedback4 = self._detrended_prices[-5]  # V4 (4 elements back from the last)

        cdef double in_phase3 = self._in_phase[-4]             # (3 elements back from the last)
        cdef double quadrature2 = self._quadrature[-3]         # (2 elements back from the last)

        # Calculate in-phase
        self._in_phase.append(
            1.25 * (feedback4 - (self._i_mult * feedback2) + (self._i_mult * in_phase3)))

        # Calculate quadrature
        self._quadrature.append(
            feedback2 - (self._q_mult * feedback1) + (self._q_mult * quadrature2))

    cdef double _calc_amplitude(self):
        """
        Calculate the signal amplitude.

        :return: The signal amplitude (dB).
        """
        return (np.power(self._in_phase[-1], 2)) + (np.power(self._quadrature[-1], 2))

    cdef double _calc_signal_noise_ratio(self):
        """
        Calculate the signal to noise ratio.

        :return: The SNR amplitude (dB).
        """
        cdef double range_squared = np.power(self._range, 2)
        return (10 * np.log(self._amplitude / range_squared)) / np.log(10) + 1.9

    cpdef void reset(self):
        """
        Reset the indicator by clearing all stateful values.
        """
        self._reset_base()
        self._inputs.clear()
        self._detrended_prices.clear()
        self._in_phase = deque([0.0] * self.period, maxlen=self.period)
        self._quadrature = deque([0.0] * self.period, maxlen=self.period)
        self._previous_range = 0.0
        self._previous_amplitude = 0.0
        self._previous_value = 0.0
        self._range = 0.0
        self._amplitude = 0.0
        self.value = 0.0
