import React, { ReactNode } from 'react';
import {
  ActivityIndicator,
  SafeAreaView,
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import { StatusBar } from 'expo-status-bar';
import { colors } from './theme';

export function Screen({ children }: { children: ReactNode }) {
  return (
    <SafeAreaView style={styles.safe}>
      <StatusBar style="dark" />
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        {children}
      </ScrollView>
    </SafeAreaView>
  );
}

type ButtonColor = 'green' | 'blue' | 'red' | 'gray';

const buttonColors: Record<ButtonColor, { bg: string; edge: string }> = {
  green: { bg: colors.green, edge: colors.greenDark },
  blue: { bg: colors.blue, edge: colors.blueDark },
  red: { bg: colors.red, edge: colors.redDark },
  gray: { bg: colors.gray, edge: colors.grayDark },
};

export function ChunkyButton({
  title,
  onPress,
  color = 'green',
  disabled = false,
  loading = false,
}: {
  title: string;
  onPress: () => void;
  color?: ButtonColor;
  disabled?: boolean;
  loading?: boolean;
}) {
  const scheme = disabled ? buttonColors.gray : buttonColors[color];
  return (
    <TouchableOpacity
      onPress={onPress}
      disabled={disabled || loading}
      style={[styles.button, { backgroundColor: scheme.bg, borderBottomColor: scheme.edge }]}
    >
      {loading ? (
        <ActivityIndicator color="#FFFFFF" />
      ) : (
        <Text style={styles.buttonText}>{title}</Text>
      )}
    </TouchableOpacity>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}) {
  return (
    <View style={styles.segmented}>
      {options.map((option) => {
        const active = option.value === value;
        return (
          <TouchableOpacity
            key={option.value}
            onPress={() => onChange(option.value)}
            style={[styles.segment, active && styles.segmentActive]}
          >
            <Text style={[styles.segmentText, active && styles.segmentTextActive]}>
              {option.label}
            </Text>
          </TouchableOpacity>
        );
      })}
    </View>
  );
}

export function Stepper({
  value,
  min,
  max,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  return (
    <View style={styles.stepper}>
      <TouchableOpacity
        onPress={() => onChange(Math.max(min, value - 1))}
        disabled={value <= min}
        style={[styles.stepperButton, value <= min && styles.stepperDisabled]}
      >
        <Text style={styles.stepperText}>−</Text>
      </TouchableOpacity>
      <Text style={styles.stepperValue}>{value}</Text>
      <TouchableOpacity
        onPress={() => onChange(Math.min(max, value + 1))}
        disabled={value >= max}
        style={[styles.stepperButton, value >= max && styles.stepperDisabled]}
      >
        <Text style={styles.stepperText}>+</Text>
      </TouchableOpacity>
    </View>
  );
}

export function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <View style={styles.field}>
      <Text style={styles.fieldLabel}>{label}</Text>
      {children}
    </View>
  );
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <View style={styles.errorBox}>
      <Text style={styles.errorText}>{message}</Text>
      {onRetry && (
        <TouchableOpacity onPress={onRetry} style={styles.errorRetry}>
          <Text style={styles.errorRetryText}>↻</Text>
        </TouchableOpacity>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  safe: {
    flex: 1,
    backgroundColor: colors.bg,
  },
  content: {
    padding: 20,
    width: '100%',
    maxWidth: 640,
    alignSelf: 'center',
  },
  button: {
    borderRadius: 12,
    borderBottomWidth: 4,
    paddingVertical: 14,
    alignItems: 'center',
  },
  buttonText: {
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '800',
    letterSpacing: 1,
  },
  segmented: {
    flexDirection: 'row',
    backgroundColor: colors.card,
    borderRadius: 12,
    borderWidth: 2,
    borderColor: colors.border,
    padding: 4,
    gap: 4,
  },
  segment: {
    flex: 1,
    borderRadius: 8,
    paddingVertical: 10,
    alignItems: 'center',
  },
  segmentActive: {
    backgroundColor: '#FFFFFF',
    borderWidth: 2,
    borderColor: colors.border,
  },
  segmentText: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.muted,
  },
  segmentTextActive: {
    color: colors.blue,
  },
  stepper: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 16,
  },
  stepperButton: {
    width: 44,
    height: 44,
    borderRadius: 12,
    backgroundColor: colors.card,
    borderWidth: 2,
    borderColor: colors.border,
    alignItems: 'center',
    justifyContent: 'center',
  },
  stepperDisabled: {
    opacity: 0.4,
  },
  stepperText: {
    fontSize: 22,
    fontWeight: '800',
    color: colors.ink,
  },
  stepperValue: {
    fontSize: 24,
    fontWeight: '800',
    color: colors.ink,
    minWidth: 48,
    textAlign: 'center',
  },
  field: {
    marginBottom: 18,
  },
  fieldLabel: {
    fontSize: 14,
    fontWeight: '800',
    color: colors.muted,
    marginBottom: 8,
    letterSpacing: 0.5,
  },
  errorBox: {
    backgroundColor: '#FFE5E5',
    borderRadius: 12,
    padding: 12,
    marginBottom: 16,
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  errorText: {
    flex: 1,
    color: colors.redDark,
    fontSize: 14,
    fontWeight: '600',
  },
  errorRetry: {
    padding: 4,
  },
  errorRetryText: {
    fontSize: 18,
    color: colors.redDark,
    fontWeight: '800',
  },
});
